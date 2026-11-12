use kb_component_runtime::{BrailleComponent, BrailleTyping};
use kb_core::{
    KeyStroke,
    preferences::{MAX_TEXT, PREFIX_TIMEOUT_MS, Settings},
    reference::{LEADER, Mode, REFERENCE, SPACE, State},
    transport::{FaultLatch, InputGate, SessionGate},
};

fn component() -> FaultLatch<BrailleComponent> {
    let bytes = wat::parse_str(
        r#"(module
            (memory (export "memory") 1)
            (func (export "kb:firmware/braille@0.1.0#map-cell") (param i32) (result i32)
                i32.const 4)
            (func (export "cabi_post_kb:firmware/braille@0.1.0#map-cell") (param i32))
            (func (export "kb:firmware/braille@0.1.0#step") (param i32 i32) (param $keys i32) (result i32)
                i32.const 0 local.get $keys i32.store16
                i32.const 4 local.get $keys i32.eqz
                if (result i32) i32.const 5 else i32.const 4 end i32.store8
                i32.const 0)
            (func (export "cabi_post_kb:firmware/braille@0.1.0#step") (param i32)))"#,
    )
    .unwrap();
    FaultLatch::new(BrailleComponent::new(&bytes))
}

fn strokes(usages: &[u8]) -> Vec<KeyStroke> {
    usages
        .iter()
        .map(|&usage| KeyStroke::new(usage, 0))
        .collect()
}

#[test]
fn normal_chord_replays_press_roll_and_release_in_order() {
    let mut typing = BrailleTyping::new(true);
    let mut component = component();
    let settings = Settings::default();
    assert!(typing.step(&mut component, &settings, 1, 0).is_empty());
    assert!(typing.step(&mut component, &settings, 3, 1).is_empty());
    assert_eq!(
        typing.step(&mut component, &settings, 0, 2).as_slice(),
        strokes(&[4, 4, 5]),
    );
}

#[test]
fn all_namespace_routes_discard_tentative_wasm_output() {
    for (command, target) in [
        (LEADER, Mode::Main),
        (REFERENCE, Mode::Reference),
        (0x109, Mode::Config),
        (0x103, Mode::Bindings),
        (0x119, Mode::Diagnostics),
    ] {
        for release in [SPACE, 0] {
            let mut component = component();
            let mut typing = BrailleTyping::new(true);
            let mut capture = State::new();
            let settings = Settings::default();
            let mut outputs = Vec::new();
            for (now, keys) in [LEADER, release, command, release, 0, 1, 0]
                .into_iter()
                .enumerate()
            {
                let next = capture.step(keys, now as u32);
                if next.reset_braille && capture.mode == Mode::Typing {
                    assert_eq!(typing.held(), LEADER);
                    typing.cancel();
                }
                if next.forward {
                    outputs.extend_from_slice(
                        typing
                            .step(&mut component, &settings, keys, now as u32)
                            .as_slice(),
                    );
                }
                capture = next.state;
            }
            assert_eq!(capture.mode, target);
            assert!(outputs.is_empty(), "{target:?} leaked {outputs:?}");
            assert!(!component.is_faulted());
        }
    }
}

#[test]
fn tentative_overflow_latches_the_wasm_fault_without_partial_output() {
    let mut typing = BrailleTyping::new(true);
    let mut component = component();
    let settings = Settings::default();
    for index in 0..MAX_TEXT {
        let keys = if index % 2 == 0 { 1 } else { 3 };
        assert!(
            typing
                .step(&mut component, &settings, keys, index as u32)
                .is_empty()
        );
        assert!(!component.is_faulted());
    }
    assert!(
        typing
            .step(&mut component, &settings, 0, MAX_TEXT as u32)
            .is_empty()
    );
    assert!(component.is_faulted());
    assert_eq!((typing.held(), typing.chord()), (0, 0));
    assert!(typing.step(&mut component, &settings, 0, 10_000).is_empty());
}

#[test]
fn link_or_host_session_change_discards_the_chord_and_contraction_prefix() {
    for link_lost in [false, true] {
        let mut typing = BrailleTyping::new(true);
        let mut component = component();
        let mut settings = Settings::default();
        settings.set_binding(0, &[1, 2], b"macro").unwrap();
        let mut input = InputGate::new();
        let mut session = SessionGate::new().set_ready(true);
        let mut feed = |session: SessionGate, keys, now, link_lost| {
            let update = input.observe(session.generation(), session.ready(), keys, link_lost);
            if update.cancel {
                typing.cancel();
            }
            update
                .keys
                .map(|keys| typing.step(&mut component, &settings, keys as u16, now))
        };
        feed(session, 0, 0, false);
        assert!(feed(session, 1, 1, false).unwrap().is_empty());
        assert!(feed(session, 0, 2, false).unwrap().is_empty());
        assert!(feed(session, 2, 3, false).unwrap().is_empty());
        if !link_lost {
            session = session.reset().set_ready(true);
        }
        assert!(feed(session, 2, 4, link_lost).is_none());
        assert!(
            feed(session, 0, PREFIX_TIMEOUT_MS + 5, false)
                .unwrap()
                .is_empty()
        );
        assert!(
            feed(session, 2, PREFIX_TIMEOUT_MS + 6, false)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            feed(session, 0, PREFIX_TIMEOUT_MS + 7, false)
                .unwrap()
                .as_slice(),
            strokes(&[4, 5]),
        );
    }
}
