use kb_core::matrix_link::{
    DISPLAY_TIMEOUT_MS, DisplayFeedback, DisplayReceiver, DisplayStatus, KEYS_MASK,
    LINK_TIMEOUT_MS, PacketError, Receiver, Side, Snapshot,
};
use kb_core::pipeline::{Action, Key};
use kb_core::report_state::ReportState;

fn snapshot(side: Side, keys: u32, sequence: u16) -> Snapshot {
    Snapshot {
        node: 17,
        boot: 42,
        sequence,
        side,
        keys,
    }
}

#[test]
fn two_halves_cover_exactly_forty_two_positions() {
    let left = Side::Left.map(KEYS_MASK);
    let right = Side::Right.map(KEYS_MASK);
    assert_eq!(left & right, 0);
    assert_eq!(left | right, (1u64 << 42) - 1);
    assert_eq!(Side::Left.position(18), Some(36));
    assert_eq!(Side::Right.position(20), Some(41));
    assert_eq!(Side::Left.matrix_index(3, 2), None);
    assert_eq!(Side::Left.matrix_index(3, 3), Some(18));
    assert_eq!(Side::Right.matrix_index(3, 2), Some(20));
    assert_eq!(Side::Right.matrix_index(3, 3), None);
}

#[test]
fn matrix_packets_round_trip_and_detect_corruption() {
    let original = snapshot(Side::Right, KEYS_MASK, u16::MAX);
    assert_eq!(Snapshot::decode(&original.encode()), Ok(original));
    for index in 0..24 {
        let mut bytes = original.encode();
        bytes[index] ^= 1;
        assert!(Snapshot::decode(&bytes).is_err());
    }
}

#[test]
fn display_feedback_round_trips_and_rejects_invalid_packets() {
    let feedback = DisplayFeedback {
        sequence: u16::MAX,
        status: DisplayStatus {
            connected: true,
            faulted: true,
            pairing_unsaved: true,
            active_keys: 42,
            layer: 1,
            last_usage: 4,
            last_modifiers: 2,
        },
    };
    let packet = feedback.encode();
    assert_eq!(DisplayFeedback::decode(&packet), Ok(feedback));
    for index in 0..packet.len() {
        let mut corrupt = packet;
        corrupt[index] ^= 1;
        assert!(DisplayFeedback::decode(&corrupt).is_err());
    }
    assert_eq!(
        DisplayFeedback::decode(&packet[..9]),
        Err(PacketError::Length)
    );
    let invalid = DisplayFeedback {
        status: DisplayStatus {
            active_keys: 43,
            ..feedback.status
        },
        ..feedback
    };
    assert_eq!(
        DisplayFeedback::decode(&invalid.encode()),
        Err(PacketError::Status)
    );
}

#[test]
fn display_feedback_expires_and_duplicates_do_not_extend_its_lifetime() {
    let mut receiver = DisplayReceiver::default();
    let feedback = DisplayFeedback {
        sequence: u16::MAX,
        status: DisplayStatus {
            connected: true,
            last_usage: 4,
            ..Default::default()
        },
    };
    let start = u32::MAX - 100;
    assert_eq!(receiver.current(start), None);
    receiver.receive(feedback, start);
    receiver.receive(feedback, 20);
    assert_eq!(
        receiver.current(start.wrapping_add(DISPLAY_TIMEOUT_MS - 1)),
        Some(feedback.status)
    );
    assert_eq!(
        receiver.current(start.wrapping_add(DISPLAY_TIMEOUT_MS)),
        None
    );
    receiver.receive(feedback, 600);
    assert_eq!(receiver.current(600), None);

    let newer = DisplayFeedback {
        sequence: 0,
        status: DisplayStatus::default(),
    };
    receiver.receive(newer, 601);
    receiver.receive(feedback, 602);
    assert_eq!(receiver.current(602), Some(newer.status));

    let mut reconnected = DisplayReceiver::default();
    assert_eq!(reconnected.current(603), None);
    reconnected.receive(newer, 604);
    assert_eq!(reconnected.current(604), Some(newer.status));
}

#[test]
fn links_require_idle_after_connect_and_reboot_and_ignore_old_packets() {
    let mut receiver = Receiver::new();
    receiver.receive(snapshot(Side::Left, 1, 0), 0).unwrap();
    assert_eq!(receiver.keys(), 0);
    receiver.receive(snapshot(Side::Left, 0, 1), 1).unwrap();
    receiver.receive(snapshot(Side::Left, 1, 2), 2).unwrap();
    assert_eq!(receiver.keys(), 1);
    receiver.receive(snapshot(Side::Left, 0, 1), 3).unwrap();
    assert_eq!(receiver.keys(), 1);
    let reboot = Snapshot {
        boot: 43,
        ..snapshot(Side::Left, 1, 0)
    };
    assert!(receiver.receive(reboot, 4).unwrap());
    assert_eq!(receiver.keys(), 0);
    assert!(receiver.expire(504));
    assert!(!receiver.connected());
}

#[test]
fn expired_queue_entries_cannot_rearm_or_resurrect_keys() {
    let mut receiver = Receiver::new();
    let arrived = u32::MAX - 100;
    let drained = arrived.wrapping_add(LINK_TIMEOUT_MS);
    for (index, keys) in [0, 1, 0].into_iter().enumerate() {
        let offset = index as u32;
        let stale = snapshot(Side::Left, keys, index as u16);
        assert_eq!(
            receiver.receive_queued(
                stale,
                arrived.wrapping_add(offset),
                drained + offset,
                index == 0
            ),
            Ok(false)
        );
        assert!(!receiver.connected());
    }

    let fresh = drained + 10;
    receiver
        .receive_queued(snapshot(Side::Left, 1, 3), fresh, fresh, false)
        .unwrap();
    assert!(receiver.connected());
    assert_eq!(receiver.keys(), 0);
    receiver
        .receive_queued(snapshot(Side::Left, 0, 4), fresh + 1, fresh + 1, false)
        .unwrap();
    receiver
        .receive_queued(snapshot(Side::Left, 1, 5), fresh + 2, fresh + 100, false)
        .unwrap();
    assert_eq!(receiver.keys(), 1);
    assert!(!receiver.expire(fresh + 2 + LINK_TIMEOUT_MS - 1));
    assert!(receiver.expire(fresh + 2 + LINK_TIMEOUT_MS));
    assert_eq!(receiver.keys(), 0);
}

#[test]
fn replacing_a_link_requires_neutral_for_the_new_identity() {
    let mut receiver = Receiver::new();
    receiver
        .receive_queued(snapshot(Side::Left, 0, 0), 500, 500, true)
        .unwrap();
    receiver
        .receive_queued(snapshot(Side::Left, 1, 1), 501, 501, false)
        .unwrap();
    let replacement = Snapshot {
        node: 18,
        ..snapshot(Side::Left, 1, 0)
    };
    assert_eq!(
        receiver.receive_queued(replacement, 1, 502, true),
        Ok(false)
    );
    assert_eq!(receiver.keys(), 1);
    assert_eq!(
        receiver.receive_queued(replacement, 503, 503, true),
        Ok(true)
    );
    assert_eq!(receiver.keys(), 0);
    assert_eq!(
        receiver.receive_queued(snapshot(Side::Left, 0, 2), 504, 504, false),
        Err(PacketError::Side)
    );
    let neutral = Snapshot {
        keys: 0,
        sequence: 1,
        ..replacement
    };
    receiver.receive_queued(neutral, 505, 505, false).unwrap();
    let pressed = Snapshot {
        sequence: 2,
        ..replacement
    };
    receiver.receive_queued(pressed, 506, 506, false).unwrap();
    assert_eq!(receiver.keys(), 1);
}

#[test]
fn rollover_recovers_as_keys_release_without_losing_modifiers() {
    let mut state = ReportState::<42>::new();
    state
        .apply(Action::Press(Key {
            usage: 225,
            modifiers: 0,
        }))
        .unwrap();
    for usage in 4..11 {
        state
            .apply(Action::Press(Key {
                usage,
                modifiers: 0,
            }))
            .unwrap();
    }
    let duplicate = Key {
        usage: 10,
        modifiers: 0,
    };
    state.apply(Action::Press(duplicate)).unwrap();
    state.apply(Action::Release(duplicate)).unwrap();
    assert_eq!(state.report(), [2, 0, 1, 1, 1, 1, 1, 1]);
    state
        .apply(Action::Release(Key {
            usage: 10,
            modifiers: 0,
        }))
        .unwrap();
    assert_eq!(state.report(), [2, 0, 4, 5, 6, 7, 8, 9]);
    state.apply(Action::ReleaseAll).unwrap();
    assert_eq!(state.report(), [0; 8]);
}

#[test]
fn held_taps_and_releases_preserve_each_exact_key_owner() {
    let mut state = ReportState::<2>::new();
    let shifted = Key {
        usage: 49,
        modifiers: 2,
    };
    let plain = Key {
        usage: 49,
        modifiers: 0,
    };
    state.apply(Action::Press(shifted)).unwrap();
    state.apply(Action::Press(shifted)).unwrap();
    state.apply(Action::Press(plain)).unwrap();
    let held = [2, 0, 49, 0, 0, 0, 0, 0];
    let tap = state.apply(Action::Tap(shifted)).unwrap();
    assert_eq!(tap.first, held);
    assert_eq!(tap.second, Some(held));
    assert_eq!(state.apply(Action::Release(shifted)).unwrap().first, held);
    assert_eq!(
        state.apply(Action::Release(shifted)).unwrap().first,
        [0, 0, 49, 0, 0, 0, 0, 0]
    );
    state.apply(Action::Press(plain)).unwrap();
    state.apply(Action::ReleaseAll).unwrap();
    state.apply(Action::Press(plain)).unwrap();
    assert_eq!(state.apply(Action::Release(plain)).unwrap().first, [0; 8]);
}

#[test]
fn per_key_modifiers_survive_other_key_releases_and_taps() {
    let mut state = ReportState::<42>::new();
    let shifted = Key {
        usage: 4,
        modifiers: 2,
    };
    let plain = Key {
        usage: 5,
        modifiers: 0,
    };
    state.apply(Action::Press(shifted)).unwrap();
    state.apply(Action::Press(plain)).unwrap();
    state.apply(Action::Release(plain)).unwrap();
    let tap = state
        .apply(Action::Tap(Key {
            usage: 44,
            modifiers: 0,
        }))
        .unwrap();
    assert_eq!(tap.first, [2, 0, 4, 44, 0, 0, 0, 0]);
    assert_eq!(tap.second, Some([2, 0, 4, 0, 0, 0, 0, 0]));
    state.apply(Action::Release(shifted)).unwrap();
    assert_eq!(state.report(), [0; 8]);
}

#[test]
fn sequence_and_clock_wrap_preserve_freshness_without_accepting_replays() {
    let mut receiver = Receiver::new();
    receiver
        .receive(snapshot(Side::Right, 0, u16::MAX), u32::MAX - 100)
        .unwrap();
    receiver
        .receive(snapshot(Side::Right, 1, 0), u32::MAX - 50)
        .unwrap();
    assert_eq!(receiver.keys(), 1 << 6);
    receiver
        .receive(snapshot(Side::Right, 0, u16::MAX), 100)
        .unwrap();
    assert_eq!(receiver.keys(), 1 << 6);
    assert!(!receiver.expire(448));
    assert!(receiver.expire(449));
    assert_eq!(receiver.keys(), 0);
    receiver.receive(snapshot(Side::Right, 1, 1), 450).unwrap();
    assert_eq!(receiver.keys(), 0);
}
