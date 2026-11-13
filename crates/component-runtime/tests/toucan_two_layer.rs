use kb_component_runtime::KeyboardComponent;
use kb_core::pipeline::{Action, Input, Key, KeyEvent, KeyState};

const KEYMAP: &[u8] =
    include_bytes!("../../../components/generated/toucan-two-layer/keyboard.core.wasm");

fn run(state: &[u8], edges: &[(u16, bool)]) -> kb_core::pipeline::Output<64, 42, 42> {
    let component = KeyboardComponent::new(KEYMAP).unwrap();
    let events: Vec<_> = edges
        .iter()
        .map(|(position, pressed)| KeyEvent {
            position: *position,
            state: if *pressed {
                KeyState::Pressed
            } else {
                KeyState::Released
            },
            timestamp_ms: 0,
        })
        .collect();
    let input = Input::<42>::with_events(0, &events).unwrap();
    component.process(state, &input).unwrap()
}

fn key(usage: u16) -> Key {
    Key {
        usage,
        modifiers: 0,
    }
}

#[test]
fn base_bindings_match_the_two_layer_zmk_layout() {
    let usages = [
        43, 20, 26, 8, 21, 23, 28, 24, 12, 18, 19, 41, 224, 4, 22, 7, 9, 10, 11, 13, 14, 15, 51,
        52, 225, 29, 27, 6, 25, 5, 17, 16, 54, 55, 56, 45, 0, 42, 227, 44, 40, 225,
    ];
    for (position, usage) in usages.into_iter().enumerate() {
        if position == 36 {
            continue;
        }
        let output = run(&[], &[(position as u16, true), (position as u16, false)]);
        assert_eq!(
            output.actions.as_slice(),
            [Action::Press(key(usage)), Action::Release(key(usage))]
        );
    }
}

#[test]
fn layer_navigation_and_modifiers_match_the_source() {
    for (position, usage) in [
        (1, 43),
        (2, 80),
        (3, 41),
        (4, 82),
        (7, 43),
        (8, 81),
        (9, 79),
        (25, 225),
        (26, 224),
        (27, 226),
        (28, 42),
        (31, 227),
        (32, 40),
        (33, 44),
        (37, 42),
        (38, 227),
        (40, 40),
    ] {
        let output = run(
            &[],
            &[(36, true), (position, true), (36, false), (position, false)],
        );
        assert_eq!(
            output.actions.as_slice(),
            [Action::Press(key(usage)), Action::Release(key(usage))]
        );
    }
}

#[test]
fn braille_bindings_are_inert_without_runtime_integration() {
    for position in [
        0, 5, 6, 10, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 29, 30, 34, 35, 39, 41,
    ] {
        let output = run(
            &[],
            &[(36, true), (position, true), (position, false), (36, false)],
        );
        assert!(output.actions.is_empty(), "position {position}");
    }
}

#[test]
fn layer_is_momentary_and_preserves_held_base_keys() {
    let output = run(
        &[],
        &[
            (16, true),
            (36, true),
            (16, false),
            (36, false),
            (16, true),
            (16, false),
        ],
    );
    assert_eq!(
        output.actions.as_slice(),
        [
            Action::Press(key(9)),
            Action::Release(key(9)),
            Action::Press(key(9)),
            Action::Release(key(9))
        ]
    );
    assert!(output.state.as_slice().iter().all(|byte| *byte == 0));
}

#[test]
fn base_placeholder_does_not_activate_braille_keys() {
    let output = run(
        &[],
        &[
            (36, true),
            (10, true),
            (10, false),
            (16, true),
            (16, false),
            (36, false),
        ],
    );
    assert!(output.actions.is_empty());
}
