use kb_component_build::{Contract, prepare};

fn compare_guest_artifacts(component: &[u8], optimized: &[u8], contract: Contract) {
    use kb_component_runtime::{BrailleComponent, DisplayGuest};
    let core = prepare(component, contract).unwrap();
    let mut browser = BrailleComponent::new(&core).unwrap();
    let mut hardware = BrailleComponent::new(optimized).unwrap();
    for cell in 0..512 {
        assert_eq!(
            browser.map_cell(cell).unwrap(),
            hardware.map_cell(cell).unwrap()
        );
    }
    for screen in 0..14 {
        let mut model = [0; 16];
        model[0] = screen;
        for (width, height) in [(144, 168), (280, 240)] {
            assert_eq!(
                browser.render(width, height, &model, "a").unwrap(),
                hardware.render(width, height, &model, "a").unwrap()
            );
        }
    }
}

#[test]
fn braille_component_and_optimized_core_render_and_type_identically() {
    let component = include_bytes!("../../components/generated/braille.component.wasm");
    compare_guest_artifacts(
        component,
        include_bytes!("../../components/generated/braille.core.wasm"),
        Contract::Braille,
    );
    assert!(prepare(component, Contract::Keyboard).is_err());
}

#[test]
fn malformed_and_core_binaries_are_rejected_as_components() {
    assert!(prepare(b"invalid", Contract::Braille).is_err());
    let core = wat::parse_str("(module)").unwrap();
    assert!(prepare(&core, Contract::Braille).is_err());
}

#[test]
fn keyboard_contract_validates_every_function_and_compound_type() {
    let bytes = include_bytes!("../../components/generated/toucan-keymap/keyboard.component.wasm");
    compare_guest_artifacts(
        bytes,
        include_bytes!("../../components/generated/toucan-keymap/keyboard.core.wasm"),
        Contract::Keyboard,
    );
    use kb_component_runtime::KeyboardComponent;
    use kb_core::pipeline::{Input, KeyEvent, KeyState};

    let browser = KeyboardComponent::new(&prepare(bytes, Contract::Keyboard).unwrap()).unwrap();
    let hardware = KeyboardComponent::new(include_bytes!(
        "../../components/generated/toucan-keymap/keyboard.core.wasm"
    ))
    .unwrap();
    let mut state = kb_core::pipeline::Bounded::<u8, 64>::default();
    for (timestamp_ms, change) in [KeyState::Pressed, KeyState::Released]
        .into_iter()
        .enumerate()
    {
        let events = core::array::from_fn::<_, 42, _>(|position| KeyEvent {
            position: position as u16,
            state: change,
            timestamp_ms: timestamp_ms as u32,
        });
        let input = Input::with_events(timestamp_ms as u32, &events).unwrap();
        let raw = browser
            .process::<64, 42, 42>(state.as_slice(), &input)
            .unwrap();
        let optimized = hardware
            .process::<64, 42, 42>(state.as_slice(), &input)
            .unwrap();
        assert_eq!(raw, optimized);
        let raw = browser
            .transform_output::<64, 42>(raw.state.as_slice(), raw.actions.as_slice())
            .unwrap();
        let optimized = hardware
            .transform_output::<64, 42>(optimized.state.as_slice(), optimized.actions.as_slice())
            .unwrap();
        assert_eq!(raw, optimized);
        state = optimized.state;
    }
    assert!(prepare(bytes, Contract::Braille).is_ok());
}

#[test]
fn exports_cannot_substitute_a_different_core_function() {
    let component = wat::parse_str(r#"(component
        (core module $module
            (memory (export "memory") 1)
            (func (export "kb:firmware/braille@0.1.0#step") (param i32 i32 i32) (result i32) i32.const 0)
            (func (export "other") (param i32 i32 i32) (result i32) i32.const 16)
            (func (export "cabi_post_kb:firmware/braille@0.1.0#step") (param i32))
            (func (export "kb:firmware/braille@0.1.0#map-cell") (param i32) (result i32) i32.const 0)
            (func (export "cabi_post_kb:firmware/braille@0.1.0#map-cell") (param i32)))
        (core instance $module (instantiate $module))
        (alias core export $module "other" (core func $function))
        (alias core export $module "memory" (core memory $memory))
        (alias core export $module "cabi_post_kb:firmware/braille@0.1.0#step" (core func $post))
        (alias core export $module "kb:firmware/braille@0.1.0#map-cell" (core func $map))
        (alias core export $module "cabi_post_kb:firmware/braille@0.1.0#map-cell" (core func $map-post))
        (type $kind (enum "idle" "tap" "sticky"))
        (type $state (record (field "held" u16) (field "chord" u16)))
        (type $output (record (field "state" $state) (field "usage" u8) (field "modifiers" u8) (field "consumer" u16) (field "kind" $kind)))
        (type $signature (func (param "state" $state) (param "keys" u16) (result $output)))
        (func $step (type $signature) (canon lift (core func $function) (memory $memory) (post-return $post)))
        (func $map-cell (param "cell" u16) (result u32) (canon lift (core func $map) (post-return $map-post)))
        (instance $exports
            (export "action-kind" (type $kind))
            (export "state" (type $state))
            (export "output" (type $output))
            (export "step" (func $step))
            (export "map-cell" (func $map-cell)))
        (export "kb:firmware/braille@0.1.0" (instance $exports)))"#).unwrap();
    wasmparser::Validator::new()
        .validate_all(&component)
        .unwrap();
    let error = prepare(&component, Contract::Braille).unwrap_err();
    assert!(error.contains("canonical function bindings"), "{error}");
}
