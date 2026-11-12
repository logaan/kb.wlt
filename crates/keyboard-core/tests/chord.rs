use kb_core::chord::CHORD_MASK;
use kb_core::{ChordError, ChordState, DotEvent, PhysicalKey};

fn edge(position: u16, bit: u16, pressed: bool) -> DotEvent {
    DotEvent {
        key: PhysicalKey {
            source: 0,
            position,
        },
        bit,
        pressed,
    }
}

#[test]
fn every_nonempty_chord_resolves_on_last_release() {
    let mut state = ChordState::<9>::new();
    for mask in 1..=CHORD_MASK {
        for index in 0..9 {
            let bit = 1 << index;
            if mask & bit != 0 {
                let (next, completed) = state.transition(edge(index, bit, true)).unwrap();
                state = next;
                assert_eq!(completed, None);
            }
        }
        assert_eq!(state.held_mask(), mask);
        for index in (0..9).rev() {
            let bit = 1 << index;
            if mask & bit != 0 {
                let (next, completed) = state.transition(edge(index, bit, false)).unwrap();
                state = next;
                assert_eq!(completed, if state.is_idle() { Some(mask) } else { None });
            }
        }
        assert!(state.is_idle());
    }
}

#[test]
fn rolling_chord_keeps_released_dots_and_late_joins() {
    let sequence = [
        edge(0, 1, true),
        edge(1, 2, true),
        edge(0, 1, false),
        edge(2, 4, true),
        edge(0, 1, true),
        edge(1, 2, false),
        edge(2, 4, false),
        edge(0, 1, false),
    ];
    let mut state = ChordState::<3>::new();
    for (index, event) in sequence.into_iter().enumerate() {
        let (next, completed) = state.transition(event).unwrap();
        state = next;
        assert_eq!(completed, if index == 7 { Some(7) } else { None });
    }
}

#[test]
fn physical_ownership_survives_duplicate_edges_and_duplicate_bindings() {
    let mut state = ChordState::<2>::new();
    for event in [
        edge(0, 1, true),
        edge(0, 1, true),
        edge(1, 1, true),
        edge(0, 1, false),
        edge(0, 1, false),
        edge(2, 1, false),
    ] {
        let (next, completed) = state.transition(event).unwrap();
        state = next;
        assert_eq!(completed, None);
    }
    let (state, completed) = state.transition(edge(1, 1, false)).unwrap();
    assert_eq!(completed, Some(1));
    assert!(state.is_idle());
}

#[test]
fn invalid_events_leave_caller_state_unchanged() {
    let state = ChordState::<1>::new()
        .transition(edge(0, 1, true))
        .unwrap()
        .0;
    for bit in [0, 3, 0x101, 0x200, u16::MAX] {
        assert_eq!(
            state.transition(edge(0, bit, true)),
            Err(ChordError::InvalidDot)
        );
    }
    assert_eq!(
        state.transition(edge(0, 2, true)),
        Err(ChordError::BindingChanged)
    );
    assert_eq!(
        state.transition(edge(0, 2, false)),
        Err(ChordError::BindingChanged)
    );
    assert_eq!(
        state.transition(edge(1, 2, true)),
        Err(ChordError::Capacity)
    );
    assert_eq!(state.transition(edge(0, 1, false)).unwrap().1, Some(1));
}

#[test]
fn discard_retains_ownership_until_every_key_is_released() {
    let mut state = ChordState::<3>::new()
        .transition(edge(0, 1, true))
        .unwrap()
        .0
        .discard();
    assert_eq!(state.held_mask(), 0);
    assert!(!state.is_idle());
    for event in [edge(1, 2, true), edge(0, 1, false), edge(1, 2, false)] {
        let (next, completed) = state.transition(event).unwrap();
        state = next;
        assert_eq!(completed, None);
    }
    state = state.transition(edge(2, 4, true)).unwrap().0;
    assert_eq!(state.transition(edge(2, 4, false)).unwrap().1, Some(4));
}

#[test]
fn split_sources_own_their_keys_and_disconnect_discards_partial_chords() {
    let mut left = edge(0, 1, true);
    let mut right = edge(0, 2, true);
    right.key.source = 1;
    let state = ChordState::<2>::new().transition(left).unwrap().0;
    let state = state.transition(right).unwrap().0;
    let state = state.disconnect(1);
    assert!(!state.is_idle());
    left.pressed = false;
    let (state, completed) = state.transition(left).unwrap();
    assert_eq!(completed, None);
    assert!(state.is_idle());
    right.pressed = false;
    assert_eq!(state.transition(right).unwrap().1, None);
}
