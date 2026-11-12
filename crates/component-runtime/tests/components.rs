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
        assert_eq!(output.stroke(), computer8(cell), "cell {cell:#05x}");
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
fn invalid_braille_actions_are_rejected_before_reaching_hid() {
    for (kind, usage, modifiers, consumer) in [
        (3, 4, 0, 0),
        (0, 4, 0, 0),
        (1, 1, 0, 0),
        (1, 0, 0, 0),
        (1, 4, 0, 0x4000),
        (2, 4, 2, 0),
        (2, 0, 0, 0),
    ] {
        let bytes = wat::parse_str(format!(r#"(module
            (memory (export "memory") 1)
            (func (export "kb:firmware/braille@0.1.0#map-cell") (param i32) (result i32) i32.const 0)
            (func (export "cabi_post_kb:firmware/braille@0.1.0#map-cell") (param i32))
            (func (export "kb:firmware/braille@0.1.0#step") (param i32 i32 i32) (result i32)
                i32.const 4 i32.const {usage} i32.store8
                i32.const 5 i32.const {modifiers} i32.store8
                i32.const 6 i32.const {consumer} i32.store16
                i32.const 8 i32.const {kind} i32.store8
                i32.const 0)
            (func (export "cabi_post_kb:firmware/braille@0.1.0#step") (param i32)))"#)).unwrap();
        let mut component = BrailleComponent::new(&bytes).unwrap();
        assert!(matches!(
            component.step(BrailleState::default(), 0),
            Err(kb_component_runtime::Error::InvalidReturn)
        ));
    }
}

#[test]
fn braille_middleware_preserves_consumer_and_action_fields() {
    let middleware = wat::parse_str(r#"(module
        (import "kb:firmware/braille@0.1.0" "step" (func $step (param i32 i32 i32 i32)))
        (import "kb:firmware/braille@0.1.0" "map-cell" (func $map (param i32) (result i32)))
        (memory (export "memory") 1)
        (func (export "kb:firmware/braille@0.1.0#step") (param i32 i32 i32) (result i32)
            local.get 0 local.get 1 local.get 2 i32.const 16 call $step i32.const 16)
        (func (export "cabi_post_kb:firmware/braille@0.1.0#step") (param i32))
        (func (export "kb:firmware/braille@0.1.0#map-cell") (param i32) (result i32) local.get 0 call $map)
        (func (export "cabi_post_kb:firmware/braille@0.1.0#map-cell") (param i32)))"#).unwrap();
    let mut component = BrailleComponent::new_chain(&[&middleware, BRAILLE]).unwrap();
    for cell in [0x150, 0x148, 0x141, 1, 0x11e] {
        let output = component.step(BrailleState::default(), cell).unwrap();
        assert_eq!(output.stroke(), None);
        let output = component.step(output.state, 0).unwrap();
        assert_eq!(output.stroke(), computer8(cell));
        assert_eq!(
            component.map_cell(cell).unwrap(),
            computer8(cell).unwrap().packed()
        );
    }
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

#[test]
fn reference_catalog_uses_the_active_component_map_cell_export() {
    let mut component = BrailleComponent::new(BRAILLE).unwrap();
    let catalog =
        kb_core::reference::Catalog::try_from_fn(|cell| component.map_cell(cell)).unwrap();
    for cell in 1..=511 {
        let actual = catalog
            .find(cell)
            .map(|index| catalog.entries()[usize::from(index)].packed);
        assert_eq!(actual, computer8(cell).map(|stroke| stroke.packed()));
    }
    for cell in 0..20_000 {
        let cell = cell % 512;
        assert_eq!(
            component.map_cell(cell).unwrap(),
            computer8(cell).map_or(0, |stroke| stroke.packed())
        );
    }
}

#[test]
fn reference_capture_suppresses_real_component_output_for_every_cell() {
    use kb_core::reference::{LEADER, REFERENCE, State};
    let mut component = BrailleComponent::new(BRAILLE).unwrap();
    for cell in 1..=511 {
        let mut reference = State::new();
        let mut state = BrailleState::default();
        let mut outputs = Vec::new();
        for (time, keys) in [0, 0x100, LEADER, 0x100, REFERENCE, 0x100, 0, cell, 0, 0]
            .into_iter()
            .enumerate()
        {
            let transition = reference.step(keys, time as u32);
            reference = transition.state;
            if transition.reset_braille {
                state = BrailleState::default();
            }
            if transition.forward && state.held != keys {
                let output = component.step(state, keys).unwrap();
                state = output.state;
                if output.usage != 0 {
                    outputs.push(output.usage);
                }
            }
        }
        assert!(outputs.is_empty(), "reference leaked cell {cell:#05x}");
    }
}

#[test]
fn reserved_namespace_and_literal_lookup_capture_custom_mapped_commands() {
    use kb_core::reference::{Catalog, LEADER, REFERENCE, State};
    let bytes = wat::parse_str(r#"(module
        (memory (export "memory") 1)
        (func (export "kb:firmware/braille@0.1.0#map-cell") (param i32) (result i32) i32.const 4)
        (func (export "cabi_post_kb:firmware/braille@0.1.0#map-cell") (param i32))
        (func (export "kb:firmware/braille@0.1.0#step") (param $held i32) (param $chord i32) (param $keys i32) (result i32)
            i32.const 0 local.get $keys i32.store16
            i32.const 2 local.get $keys i32.eqz
            if (result i32) i32.const 0 else local.get $chord local.get $keys i32.or end i32.store16
            i32.const 4 local.get $keys i32.eqz
            if (result i32) i32.const 4 else i32.const 0 end i32.store8
            i32.const 8 local.get $keys i32.eqz i32.store8
            i32.const 0)
        (func (export "cabi_post_kb:firmware/braille@0.1.0#step") (param i32)))"#).unwrap();
    let mut component = BrailleComponent::new(&bytes).unwrap();
    let catalog = Catalog::try_from_fn(|cell| component.map_cell(cell)).unwrap();
    assert_eq!(catalog.len(), 511);
    assert!(catalog.find(LEADER).is_some());
    let mut reference = State::new();
    let mut state = BrailleState::default();
    let mut time = 0;
    let mut outputs = Vec::new();
    let mut keys = |keys| {
        time += 1;
        let transition = reference.step(keys, time);
        reference = transition.state;
        if transition.reset_braille {
            state = BrailleState::default();
        }
        if transition.forward && state.held != keys {
            let output = component.step(state, keys).unwrap();
            state = output.state;
            if output.usage != 0 {
                outputs.push(output.usage);
            }
        }
    };
    for cell in [LEADER, REFERENCE] {
        keys(cell);
        keys(0);
    }
    for cell in 1..=511 {
        keys(0x10b);
        keys(0);
        keys(cell);
        keys(0);
    }
    keys(0x103);
    keys(0);
    keys(0);
    keys(1);
    keys(0);
    assert_eq!(outputs, [4]);
}
