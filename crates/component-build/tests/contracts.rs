use kb_component_build::{Contract, prepare};

#[test]
fn validated_led_component_extracts_committed_core() {
    let bytes = include_bytes!("../../../components/generated/led.component.wasm");
    assert_eq!(
        prepare(bytes, Contract::Led).unwrap(),
        include_bytes!("../../../components/generated/led.core.wasm")
    );
    assert!(prepare(bytes, Contract::Braille).is_err());
}

#[test]
fn validated_braille_component_extracts_committed_core() {
    let bytes = include_bytes!("../../../components/generated/braille.component.wasm");
    assert_eq!(
        prepare(bytes, Contract::Braille).unwrap(),
        include_bytes!("../../../components/generated/braille.core.wasm")
    );
    assert!(prepare(bytes, Contract::Led).is_err());
}

#[test]
fn malformed_and_core_binaries_are_rejected_as_components() {
    assert!(prepare(b"invalid", Contract::Led).is_err());
    let core = wat::parse_str("(module)").unwrap();
    assert!(prepare(&core, Contract::Led).is_err());
}
