use kb_core::matrix_link::{KEYS_MASK, Receiver, Side, Snapshot};
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
