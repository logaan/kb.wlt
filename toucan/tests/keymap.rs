use kb_component_runtime::KeyboardComponent;
use kb_core::pipeline::{Action, Input, Key, KeyEvent, KeyState};

const KEYMAP: &[u8] =
    include_bytes!("../../clients/components/generated/toucan-keymap/keyboard.core.wasm");

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
fn typing_bindings_match_the_base_layout() {
    let usages = [
        43, 20, 26, 8, 21, 23, 28, 24, 12, 18, 19, 41, 224, 4, 22, 7, 9, 10, 11, 13, 14, 15, 51,
        52, 225, 29, 27, 6, 25, 5, 17, 16, 54, 55, 56, 45, 0, 42, 227, 44, 40, 0,
    ];
    for (position, usage) in usages.into_iter().enumerate() {
        let output = run(&[], &[(position as u16, true), (position as u16, false)]);
        if usage == 0 {
            assert!(output.actions.is_empty());
        } else {
            assert_eq!(
                output.actions.as_slice(),
                [Action::Press(key(usage)), Action::Release(key(usage))]
            );
        }
    }
}

#[test]
fn braille_thumbs_do_not_change_conventional_key_usages() {
    for thumb in [36, 41] {
        let output = run(&[], &[(thumb, true), (2, true), (thumb, false), (2, false)]);
        assert_eq!(
            output.actions.as_slice(),
            [Action::Press(key(26)), Action::Release(key(26))]
        );
    }
}
