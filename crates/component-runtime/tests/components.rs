use kb_component_runtime::{BrailleComponent, BrailleState, LedComponent, LedState};
use kb_core::braille::computer8;

const LED: &[u8] = include_bytes!("../../../components/generated/led.core.wasm");
const BRAILLE: &[u8] = include_bytes!("../../../components/generated/braille.core.wasm");

#[test]
fn led_cycles_every_colour_and_returns_explicit_state() {
    let mut component = LedComponent::new(LED).unwrap();
    let mut state = LedState::default();
    for (second, rgb) in [
        (255, 0, 0),
        (255, 255, 0),
        (0, 255, 0),
        (0, 255, 255),
        (0, 0, 255),
        (255, 0, 255),
        (255, 0, 0),
    ]
    .into_iter()
    .enumerate()
    {
        let output = component.step(state, second as u32 * 1000, false).unwrap();
        assert_eq!((output.red, output.green, output.blue), rgb);
        assert_eq!(output.state.last_change_ms, second as u32 * 1000);
        state = output.state;
    }
}

#[test]
fn repeated_calls_release_the_guest_arena() {
    let mut component = LedComponent::new(LED).unwrap();
    let mut state = LedState::default();
    for time in 0..20_000 {
        state = component.step(state, time, false).unwrap().state;
    }
    assert_eq!(state.phase, 1);
}

#[test]
fn led_handles_clock_wrap_and_button_edges() {
    let mut component = LedComponent::new(LED).unwrap();
    let initial = LedState {
        last_change_ms: u32::MAX - 200,
        ..LedState::default()
    };
    let output = component.step(initial, 300, false).unwrap();
    assert_eq!(output.state.phase, 0);
    let output = component.step(output.state, 800, false).unwrap();
    assert_eq!(output.state.phase, 1);
    let output = component.step(output.state, 801, true).unwrap();
    assert_eq!(output.state.speed, 1);
    let output = component.step(output.state, 802, true).unwrap();
    assert_eq!(output.state.speed, 1);
}

#[test]
fn every_braille_cell_matches_the_reference_keyboard() {
    let mut component = BrailleComponent::new(BRAILLE).unwrap();
    for cell in 1..=511 {
        let output = component.step(BrailleState::default(), cell).unwrap();
        assert_eq!((output.usage, output.modifiers), (0, 0));
        let output = component.step(output.state, 0).unwrap();
        let expected = computer8(cell).unwrap_or_default();
        assert_eq!(
            (output.usage, output.modifiers),
            (expected.usage, expected.modifiers),
            "cell {cell:#05x}"
        );
        assert_eq!(output.state, BrailleState::default());
        let output = component.step(output.state, 0).unwrap();
        assert_eq!(output.usage, 0);
    }
}

#[test]
fn rolled_chords_accumulate_and_emit_once_on_final_release() {
    let mut component = BrailleComponent::new(BRAILLE).unwrap();
    let mut state = BrailleState::default();
    for keys in [1, 3, 2, 6, 4] {
        let output = component.step(state, keys).unwrap();
        assert_eq!(output.usage, 0);
        state = output.state;
    }
    assert_eq!(state.chord, 7);
    let output = component.step(state, 0).unwrap();
    assert_eq!(output.usage, computer8(7).unwrap().usage);
    assert_eq!(component.step(output.state, 0).unwrap().usage, 0);
}

#[test]
fn physical_space_and_space_dot_seven_produce_space_and_return() {
    let mut component = BrailleComponent::new(BRAILLE).unwrap();
    for (keys, usage) in [(0x100, 44), (0x140, 40), (0x40, 42)] {
        let output = component.step(BrailleState::default(), keys).unwrap();
        assert_eq!(component.step(output.state, 0).unwrap().usage, usage);
    }
}

#[test]
fn component_state_can_be_replayed_independently() {
    let mut component = BrailleComponent::new(BRAILLE).unwrap();
    let state = BrailleState { held: 1, chord: 1 };
    let first = component.step(state, 3).unwrap();
    component.step(BrailleState::default(), 0x100).unwrap();
    assert_eq!(first, component.step(state, 3).unwrap());
}

#[test]
fn guest_execution_is_fuel_limited() {
    let module = wat::parse_str(
        r#"(module
        (memory (export "memory") 1)
        (func (export "kb:firmware/led@0.1.0#step") (param i32 i32 i32 i32 i32 i32) (result i32)
            (loop $again br $again) i32.const 0)
        (func (export "cabi_post_kb:firmware/led@0.1.0#step") (param i32)))"#,
    )
    .unwrap();
    let mut component = LedComponent::new(&module).unwrap();
    assert!(component.step(LedState::default(), 0, false).is_err());
}

#[test]
fn excessive_guest_memory_is_rejected() {
    let module = wat::parse_str("(module (memory (export \"memory\") 2))").unwrap();
    assert!(LedComponent::new(&module).is_err());
}

#[test]
fn composed_led_middleware_transforms_outgoing_colours() {
    let mut component = LedComponent::new_chain(&[
        include_bytes!("../../../components/generated/led-dim/stage0/led.core.wasm"),
        include_bytes!("../../../components/generated/led-dim/stage1/led.core.wasm"),
    ])
    .unwrap();
    let mut state = LedState::default();
    for (second, rgb) in [
        (63, 0, 0),
        (63, 63, 0),
        (0, 63, 0),
        (0, 63, 63),
        (0, 0, 63),
        (63, 0, 63),
    ]
    .into_iter()
    .enumerate()
    {
        let output = component.step(state, second as u32 * 1000, false).unwrap();
        assert_eq!((output.red, output.green, output.blue), rgb);
        state = output.state;
    }
    for time in 6000..16000 {
        state = component.step(state, time, false).unwrap().state;
    }
}
