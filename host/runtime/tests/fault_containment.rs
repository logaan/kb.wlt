use kb_component_runtime::{BrailleComponent, BrailleState, KeyboardComponent};
use kb_core::pipeline::Input;
use kb_core::transport::{Fault, FaultLatch};

#[test]
fn exhausted_guest_fuel_latches_the_instance_until_replacement() {
    let module = wat::parse_str(
        r#"(module
        (memory (export "memory") 1)
        (func (export "kb:firmware/braille@0.1.0#map-cell") (param i32) (result i32) i32.const 0)
        (func (export "cabi_post_kb:firmware/braille@0.1.0#map-cell") (param i32))
        (func (export "kb:firmware/braille@0.1.0#step") (param i32 i32 i32) (result i32)
            (loop $again br $again) i32.const 0)
        (func (export "cabi_post_kb:firmware/braille@0.1.0#step") (param i32)))"#,
    )
    .unwrap();
    let mut component = FaultLatch::new(BrailleComponent::new(&module));
    let trapped = component.call(|component| component.step(BrailleState::default(), 0));
    assert!(matches!(trapped, Err(Fault::Trapped(_))));
    assert!(component.is_faulted());
    let retried = component.call(|_| -> Result<(), kb_component_runtime::Error> {
        panic!("a trapped module must not execute again");
    });
    assert!(matches!(retried, Err(Fault::Latched)));
}

#[test]
fn malformed_keyboard_output_latches_before_actions_reach_the_host() {
    let module = wat::parse_str(
        r#"(module
        (memory (export "memory") 1)
        (func (export "cabi_realloc") (param i32 i32 i32 i32) (result i32) i32.const 1024)
        (func (export "kb:firmware/keyboard@0.1.0#process") (param i32 i32 i32 i32 i32) (result i32) i32.const -1)
        (func (export "kb:firmware/keyboard@0.1.0#transform-output") (param i32 i32 i32 i32) (result i32) i32.const 0)
        (func (export "cabi_post_kb:firmware/keyboard@0.1.0#process") (param i32))
        (func (export "cabi_post_kb:firmware/keyboard@0.1.0#transform-output") (param i32)))"#,
    )
    .unwrap();
    let mut component = FaultLatch::new(KeyboardComponent::new(&module));
    let result = component.call(|component| component.process::<64, 42, 42>(&[], &Input::tick(0)));
    assert!(matches!(result, Err(Fault::Trapped(_))));
    assert!(component.is_faulted());
    let subsequent =
        component.call(|component| component.process::<64, 42, 42>(&[], &Input::tick(1)));
    assert!(matches!(subsequent, Err(Fault::Latched)));
}
