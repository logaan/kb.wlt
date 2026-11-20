use kb_core::matrix_link::{
    BRAILLE_THUMB, BRAILLE_THUMBS, BrailleSwitch, DisplayFeedback, KEYS_MASK, LINK_TIMEOUT_MS,
    PacketError, Receiver, Side, Snapshot,
};
use kb_core::pipeline::{Action, Key};
use kb_core::report_state::ReportState;

#[test]
fn tapping_the_outer_thumb_toggles_braille_and_holding_is_momentary() {
    let thumb = 1 << BRAILLE_THUMB;
    let mut mode = BrailleSwitch::default();
    assert!(!mode.update(0, Some(thumb)));
    assert!(mode.update(100, Some(0)));
    assert!(mode.update(200, Some(thumb)));
    assert!(mode.update(400, Some(thumb)));
    assert!(mode.update(401, Some(0)));
    assert!(mode.update(500, Some(thumb)));
    assert!(!mode.update(600, Some(0)));
    assert!(!mode.update(700, Some(thumb)));
    assert!(mode.update(900, Some(thumb)));
    assert!(!mode.update(901, Some(0)));
}

#[test]
fn a_chord_activates_braille_early_and_disconnect_cancels_the_hold() {
    let thumb = 1 << BRAILLE_THUMB;
    let mut mode = BrailleSwitch::default();
    assert!(!mode.update(u32::MAX, Some(thumb)));
    assert!(mode.update(1, Some(thumb | (1 << 16))));
    assert!(!mode.update(2, Some(0)));
    assert!(!mode.update(10, Some(thumb)));
    assert!(mode.update(210, Some(thumb)));
    assert!(!mode.update(211, None));
    assert!(!mode.update(212, Some(0)));
}

#[test]
fn either_outer_thumb_can_latch_or_hold_braille() {
    for thumb in [1 << BRAILLE_THUMB, 1 << 41] {
        let mut mode = BrailleSwitch::default();
        assert!(!mode.update(10, Some(thumb)));
        assert!(!mode.update(209, Some(thumb)));
        assert!(mode.update(210, Some(thumb)));
        assert!(!mode.update(211, Some(0)));
        assert!(!mode.update(220, Some(thumb)));
        assert!(mode.update(221, Some(0)));
    }
}

#[test]
fn overlapping_outer_thumbs_behave_like_one_key() {
    let left = 1 << BRAILLE_THUMB;
    let right = 1 << 41;
    let mut mode = BrailleSwitch::default();
    assert!(!mode.update(10, Some(left)));
    assert!(!mode.update(20, Some(BRAILLE_THUMBS)));
    assert!(!mode.update(30, Some(right)));
    assert!(mode.update(40, Some(0)));
    assert!(mode.update(200, Some(right)));
    assert!(mode.update(250, Some(BRAILLE_THUMBS)));
    assert!(mode.update(310, Some(left)));
    assert!(mode.update(400, Some(left)));
    assert!(mode.update(410, Some(0)));
    assert!(mode.update(420, Some(left)));
    assert!(!mode.update(430, Some(0)));
}

#[test]
fn home_row_and_space_map_to_braille_and_qq_opens_main_menu() {
    use kb_core::{
        matrix_link::braille_keys,
        reference::{Mode, State},
    };
    assert_eq!(braille_keys((1 << BRAILLE_THUMB) | (1 << 10)), 0);
    let q = [14, 15, 16, 19, 20]
        .into_iter()
        .fold(0, |keys, pos| keys | 1 << pos);
    let space = 1 << 39;
    let mut state = State::new();
    for (now, keys) in [space, space | q, space, space | q, space, 0]
        .into_iter()
        .enumerate()
    {
        state = state.step(braille_keys(keys), now as u32).state;
    }
    assert_eq!(state.mode, Mode::Main);
}

fn snapshot(side: Side, keys: u32, sequence: u16) -> Snapshot {
    Snapshot {
        node: 17,
        boot: 42,
        sequence,
        side,
        keys,
        battery: 75,
    }
}

#[test]
fn two_halves_cover_exactly_forty_two_positions() {
    let left = Side::Left.map(KEYS_MASK);
    let right = Side::Right.map(KEYS_MASK);
    assert_eq!(left & right, 0);
    assert_eq!(left | right, (1u64 << 42) - 1);
    assert_eq!(Side::Left.position(18), Some(BRAILLE_THUMB));
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
        model: kb_core::pipeline::Bounded::from_slice(&[1, 2, 3], 0).unwrap(),
        detail: kb_core::pipeline::Bounded::from_slice("⠭".as_bytes(), 0).unwrap(),
    };
    let packet = feedback.encode();
    assert_eq!(DisplayFeedback::decode(packet.as_slice()), Ok(feedback));
    for index in 0..packet.len() {
        let mut corrupt = packet;
        corrupt.as_mut_slice()[index] ^= 1;
        assert!(DisplayFeedback::decode(corrupt.as_slice()).is_err());
    }
    assert_eq!(
        DisplayFeedback::decode(&packet.as_slice()[..9]),
        Err(PacketError::Length)
    );
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

#[test]
fn raw_left_thumb_and_dot_pins_join_the_right_snapshot() {
    use kb_core::matrix_link::{braille_keys, joined_matrix};

    let left = (1 << 18) | (1 << 10);
    let right = Side::Right.map(1 << 9);
    let matrix = joined_matrix(left, right);
    assert_eq!(matrix & (1 << BRAILLE_THUMB), 1 << BRAILLE_THUMB);
    assert_eq!(braille_keys(matrix), 0b0010_0001);
}

#[test]
fn either_half_can_enter_all_eight_dots() {
    use kb_core::matrix_link::{braille_keys, joined_matrix};

    let left = [10, 9, 8, 16, 15, 14, 7, 13];
    let right = [13, 14, 15, 7, 8, 9, 16, 10];
    for cell in 0..=255 {
        let half_mask = |positions: [u32; 8]| {
            positions
                .into_iter()
                .enumerate()
                .filter(|(dot, _)| cell & (1 << dot) != 0)
                .fold(0, |keys, (_, position)| keys | (1 << position))
        };
        let left = half_mask(left);
        let right = Side::Right.map(half_mask(right));
        assert_eq!(braille_keys(joined_matrix(left, 0)), cell);
        assert_eq!(braille_keys(joined_matrix(0, right)), cell);
        assert_eq!(braille_keys(joined_matrix(left, right)), cell);
    }
    assert_eq!(braille_keys(Side::Right.map(1 << 18)), 1 << 8);
}

#[test]
fn either_inner_thumb_can_enter_space_and_lead_a_menu_chord() {
    use kb_core::{
        matrix_link::{braille_keys, joined_matrix},
        reference::{Mode, State},
    };

    let left_q = 0b0001_1100 << 6 | 0b011 << 15;
    let right_q = Side::Right.map(0b11 << 7 | 0b111 << 13);
    let left_space = 1 << 20;
    let right_space = Side::Right.map(1 << 18);
    for (left, right) in [(left_space, 0), (0, right_space), (left_space, right_space)] {
        assert_eq!(braille_keys(joined_matrix(left, right)), 1 << 8);
        for (dot_left, dot_right) in [(left_q, 0), (0, right_q), (left_q, right_q)] {
            let space = joined_matrix(left, right);
            let chord = space | joined_matrix(dot_left, dot_right);
            let mut state = State::new();
            for (time, keys) in [space, chord, space, chord, space, 0]
                .into_iter()
                .enumerate()
            {
                state = state.step(braille_keys(keys), time as u32).state;
            }
            assert_eq!(state.mode, Mode::Main);
        }
    }
}

#[test]
fn duplicate_dot_stays_pressed_when_its_other_key_releases() {
    use kb_core::matrix_link::{braille_keys, joined_matrix};

    let left_dot_one = 1 << 10;
    let right_dot_one = Side::Right.map(1 << 13);
    let right_dot_six = Side::Right.map(1 << 9);
    let dot_one_and_six = 0b0010_0001;
    let sampled = [
        (left_dot_one, 0, 1),
        (left_dot_one, right_dot_one | right_dot_six, dot_one_and_six),
        (0, right_dot_one | right_dot_six, dot_one_and_six),
        (0, right_dot_six, 0b0010_0000),
        (0, 0, 0),
    ];
    for (left, right, cell) in sampled {
        assert_eq!(braille_keys(joined_matrix(left, right)), cell);
    }
}

#[test]
fn entering_momentary_braille_keeps_the_held_thumb_visible_between_scans() {
    use kb_core::matrix_link::{
        BrailleSwitch, braille_keys, braille_transition_blocked, joined_matrix,
    };

    let thumb = joined_matrix(1 << 18, 0);
    let mut layer = BrailleSwitch::default();
    assert!(!layer.update(0, Some(thumb)));
    assert!(!layer.update(190, Some(thumb)));
    assert!(layer.update(200, Some(thumb)));
    let blocked = braille_transition_blocked(true, thumb, thumb);
    assert_eq!(blocked, 0);
    assert!(layer.update(205, Some(thumb & !blocked)));
    let with_dot = joined_matrix((1 << 18) | (1 << 10), 0);
    assert!(layer.update(215, Some(with_dot & !blocked)));
    assert_eq!(braille_keys(with_dot), 1);
    assert!(!layer.update(220, Some(with_dot & !thumb)));
    assert_eq!(
        braille_transition_blocked(false, with_dot, with_dot & !thumb),
        with_dot & !thumb
    );
    assert!(!layer.update(230, Some(0)));
}
