use kb_component_build::{Contract, prepare};

#[test]
fn validated_led_component_extracts_committed_core() {
    let bytes = include_bytes!("../../components/generated/led.component.wasm");
    assert_eq!(
        prepare(bytes, Contract::Led).unwrap(),
        include_bytes!("../../components/generated/led.core.wasm")
    );
    assert!(prepare(bytes, Contract::Braille).is_err());
}

#[test]
fn validated_braille_component_extracts_committed_core() {
    let bytes = include_bytes!("../../components/generated/braille.component.wasm");
    assert_eq!(
        prepare(bytes, Contract::Braille).unwrap(),
        include_bytes!("../../components/generated/braille.core.wasm")
    );
    assert!(prepare(bytes, Contract::Led).is_err());
}

#[test]
fn malformed_and_core_binaries_are_rejected_as_components() {
    assert!(prepare(b"invalid", Contract::Led).is_err());
    let core = wat::parse_str("(module)").unwrap();
    assert!(prepare(&core, Contract::Led).is_err());
}

#[test]
fn keyboard_contract_validates_every_function_and_compound_type() {
    let bytes = include_bytes!("../../components/generated/toucan-keymap/keyboard.component.wasm");
    assert_eq!(
        prepare(bytes, Contract::Keyboard).unwrap(),
        include_bytes!("../../components/generated/toucan-keymap/keyboard.core.wasm")
    );
    assert!(prepare(bytes, Contract::Led).is_err());
}

#[test]
fn exports_cannot_substitute_a_different_core_function() {
    let component = wat::parse_str(r#"(component
        (core module $module
            (memory (export "memory") 1)
            (func (export "kb:firmware/led@0.1.0#step") (param i32 i32 i32 i32 i32 i32) (result i32) i32.const 0)
            (func (export "other") (param i32 i32 i32 i32 i32 i32) (result i32) i32.const 16)
            (func (export "cabi_post_kb:firmware/led@0.1.0#step") (param i32)))
        (core instance $module (instantiate $module))
        (alias core export $module "other" (core func $function))
        (alias core export $module "memory" (core memory $memory))
        (alias core export $module "cabi_post_kb:firmware/led@0.1.0#step" (core func $post))
        (type $state (record (field "phase" u8) (field "speed" u8) (field "last-change-ms" u32) (field "button-was-down" bool)))
        (type $output (record (field "state" $state) (field "red" u8) (field "green" u8) (field "blue" u8)))
        (type $signature (func (param "state" $state) (param "now-ms" u32) (param "button-down" bool) (result $output)))
        (func $step (type $signature) (canon lift (core func $function) (memory $memory) (post-return $post)))
        (instance $exports
            (export "state" (type $state))
            (export "output" (type $output))
            (export "step" (func $step)))
        (export "kb:firmware/led@0.1.0" (instance $exports)))"#).unwrap();
    wasmparser::Validator::new()
        .validate_all(&component)
        .unwrap();
    let error = prepare(&component, Contract::Led).unwrap_err();
    assert!(error.contains("canonical function bindings"), "{error}");
}
