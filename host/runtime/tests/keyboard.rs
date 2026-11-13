use kb_component_runtime::KeyboardComponent;
use kb_core::pipeline::{Action, Input, Key, KeyEvent, KeyState};
use kb_core::report_state::ReportState;

const PASSTHROUGH: &[u8] =
    include_bytes!("../../../clients/components/generated/passthrough/keyboard.core.wasm");

#[test]
fn component_round_trips_explicit_state_and_events() {
    let component = KeyboardComponent::new(PASSTHROUGH).unwrap();
    let events = [
        KeyEvent {
            position: 65535,
            state: KeyState::Pressed,
            timestamp_ms: u32::MAX,
        },
        KeyEvent {
            position: 42,
            state: KeyState::Released,
            timestamp_ms: 10,
        },
    ];
    let input = Input::<8>::with_events(10, &events).unwrap();
    let result = component
        .process::<16, 8, 8>(&[0, 128, 255], &input)
        .unwrap();
    assert_eq!(result.state.as_slice(), &[0, 128, 255]);
    assert_eq!(result.events.as_slice(), &events);
    assert!(result.actions.is_empty());
    assert_eq!(result.wake_at_ms, None);
}

#[test]
fn component_round_trips_all_action_variants() {
    let component = KeyboardComponent::new(PASSTHROUGH).unwrap();
    let key = Key {
        usage: u16::MAX,
        modifiers: 255,
    };
    let actions = [
        Action::Press(key),
        Action::Release(key),
        Action::Tap(key),
        Action::ReleaseAll,
    ];
    let result = component
        .transform_output::<16, 8>(&[1, 2, 3], &actions)
        .unwrap();
    assert_eq!(result.state.as_slice(), &[1, 2, 3]);
    assert_eq!(result.actions.as_slice(), &actions);
}

#[test]
fn empty_lists_and_repeated_calls_do_not_leak_guest_arena() {
    let component = KeyboardComponent::new(PASSTHROUGH).unwrap();
    for now in 0..2000 {
        let result = component
            .process::<16, 8, 8>(&[], &Input::tick(now))
            .unwrap();
        assert!(result.state.is_empty());
        assert!(result.events.is_empty());
        assert!(result.actions.is_empty());
        let result = component.transform_output::<16, 8>(&[], &[]).unwrap();
        assert!(result.state.is_empty());
        assert!(result.actions.is_empty());
    }
}

const TOUCAN: &[u8] =
    include_bytes!("../../../clients/components/generated/toucan-keymap/keyboard.core.wasm");

fn event(position: u16, state: KeyState) -> KeyEvent {
    KeyEvent {
        position,
        state,
        timestamp_ms: 0,
    }
}

#[test]
fn toucan_keymap_emits_qwerty_presses_and_releases() {
    let component = KeyboardComponent::new(TOUCAN).unwrap();
    let input = Input::<8>::with_events(
        0,
        &[
            event(1, KeyState::Pressed),
            event(13, KeyState::Pressed),
            event(38, KeyState::Pressed),
        ],
    )
    .unwrap();
    let output = component.process::<64, 8, 8>(&[], &input).unwrap();
    assert_eq!(
        output.actions.as_slice(),
        &[
            Action::Press(Key {
                usage: 20,
                modifiers: 0
            }),
            Action::Press(Key {
                usage: 4,
                modifiers: 0
            }),
            Action::Press(Key {
                usage: 44,
                modifiers: 0
            }),
        ]
    );
    assert_eq!(output.state.len(), 42);
    assert!(output.events.is_empty());
    let input = Input::<8>::with_events(
        1,
        &[
            event(1, KeyState::Released),
            event(13, KeyState::Released),
            event(38, KeyState::Released),
        ],
    )
    .unwrap();
    let output = component
        .process::<64, 8, 8>(output.state.as_slice(), &input)
        .unwrap();
    assert_eq!(
        output.actions.as_slice(),
        &[
            Action::Release(Key {
                usage: 20,
                modifiers: 0
            }),
            Action::Release(Key {
                usage: 4,
                modifiers: 0
            }),
            Action::Release(Key {
                usage: 44,
                modifiers: 0
            }),
        ]
    );
    assert!(output.state.as_slice().iter().all(|byte| *byte == 0));
}

#[test]
fn toucan_layers_preserve_the_original_key_when_released() {
    let component = KeyboardComponent::new(TOUCAN).unwrap();
    let input = Input::<8>::with_events(
        0,
        &[
            event(41, KeyState::Pressed),
            event(1, KeyState::Pressed),
            event(41, KeyState::Released),
            event(1, KeyState::Released),
        ],
    )
    .unwrap();
    let output = component.process::<64, 8, 8>(&[], &input).unwrap();
    assert_eq!(
        output.actions.as_slice(),
        &[
            Action::Press(Key {
                usage: 30,
                modifiers: 0
            }),
            Action::Release(Key {
                usage: 30,
                modifiers: 0
            }),
        ]
    );
}

#[test]
fn toucan_ignores_duplicate_edges_and_passes_unmapped_positions() {
    let component = KeyboardComponent::new(TOUCAN).unwrap();
    let events = [
        event(1, KeyState::Pressed),
        event(1, KeyState::Pressed),
        event(1, KeyState::Released),
        event(1, KeyState::Released),
        event(42, KeyState::Pressed),
    ];
    let input = Input::<8>::with_events(0, &events).unwrap();
    let output = component.process::<64, 8, 8>(&[], &input).unwrap();
    assert_eq!(output.actions.len(), 2);
    assert_eq!(output.events.as_slice(), &[events[4]]);
}

#[test]
fn toucan_accepts_all_physical_keys_in_one_scan() {
    let component = KeyboardComponent::new(TOUCAN).unwrap();
    let events: Vec<_> = (0..42)
        .map(|position| event(position, KeyState::Pressed))
        .collect();
    let input = Input::<42>::with_events(0, &events).unwrap();
    let output = component.process::<64, 42, 42>(&[], &input).unwrap();
    assert_eq!(output.actions.len(), 41);
}

#[test]
fn toucan_duplicate_usage_survives_either_physical_release_order() {
    use kb_component_runtime::KeyboardPipeline;
    use kb_core::pipeline::PipelineState;

    let component = KeyboardComponent::new(TOUCAN).unwrap();
    let composed = KeyboardPipeline::new(&[
        include_bytes!("../../../clients/components/generated/keyboard-composed/stage0/keyboard.core.wasm"),
        include_bytes!("../../../clients/components/generated/keyboard-composed/stage1/keyboard.core.wasm"),
    ])
    .unwrap();
    for (first, last) in [(11, 25), (25, 11)] {
        let input = Input::<6>::with_events(
            0,
            &[
                event(41, KeyState::Pressed),
                event(11, KeyState::Pressed),
                event(25, KeyState::Pressed),
                event(41, KeyState::Released),
                event(first, KeyState::Released),
                event(last, KeyState::Released),
            ],
        )
        .unwrap();
        let plain = component.process::<64, 6, 4>(&[], &input).unwrap();
        let shifted = composed
            .process::<64, 6, 4>(PipelineState::new(), input)
            .unwrap();
        for actions in [plain.actions, shifted.actions] {
            let key = Key {
                usage: 49,
                modifiers: 0,
            };
            assert_eq!(
                actions.as_slice(),
                [
                    Action::Press(key),
                    Action::Press(key),
                    Action::Release(key),
                    Action::Release(key)
                ]
            );
            let mut reports = ReportState::<42>::new();
            let held = [0, 0, 49, 0, 0, 0, 0, 0];
            for (action, expected) in actions.as_slice().iter().zip([held, held, held, [0; 8]]) {
                assert_eq!(reports.apply(*action).unwrap().first, expected);
            }
        }
    }
}

#[test]
fn pipeline_composes_wavelet_input_and_output_middleware() {
    use kb_core::pipeline::{Middleware, Pipeline, PipelineState};
    let shift = KeyboardComponent::new(include_bytes!(
        "../../../clients/components/generated/shift/keyboard.core.wasm"
    ))
    .unwrap();
    let keymap = KeyboardComponent::new(TOUCAN).unwrap();
    let stages: [&dyn Middleware<64, 8, 8>; 2] = [&shift, &keymap];
    let pipeline = Pipeline::new(stages);
    let input = Input::<8>::with_events(
        0,
        &[
            event(1, KeyState::Pressed),
            event(1, KeyState::Released),
            event(38, KeyState::Pressed),
        ],
    )
    .unwrap();
    let output = pipeline.process(PipelineState::new(), input).unwrap();
    assert_eq!(
        output.actions.as_slice(),
        &[
            Action::Press(Key {
                usage: 20,
                modifiers: 2
            }),
            Action::Release(Key {
                usage: 20,
                modifiers: 2
            }),
            Action::Press(Key {
                usage: 44,
                modifiers: 0
            }),
        ]
    );
    assert!(output.state.stages[0].is_empty());
    assert_eq!(output.state.stages[1].len(), 42);
}

#[test]
fn owning_pipeline_composes_optimized_components() {
    use kb_component_runtime::KeyboardPipeline;
    use kb_core::pipeline::PipelineState;
    let pipeline = KeyboardPipeline::new(&[
        include_bytes!("../../../clients/components/generated/keyboard-composed/stage0/keyboard.core.wasm"),
        include_bytes!("../../../clients/components/generated/keyboard-composed/stage1/keyboard.core.wasm"),
    ])
    .unwrap();
    let input = Input::<8>::with_events(
        0,
        &[
            event(1, KeyState::Pressed),
            event(1, KeyState::Released),
            event(38, KeyState::Pressed),
        ],
    )
    .unwrap();
    let output = pipeline
        .process::<64, 8, 8>(PipelineState::new(), input)
        .unwrap();
    assert_eq!(
        output.actions.as_slice(),
        &[
            Action::Press(Key {
                usage: 20,
                modifiers: 2
            }),
            Action::Release(Key {
                usage: 20,
                modifiers: 2
            }),
            Action::Press(Key {
                usage: 44,
                modifiers: 0
            }),
        ]
    );
    assert!(output.state.stages[0].is_empty());
    assert_eq!(output.state.stages[1].len(), 42);
}

#[test]
fn owning_pipeline_rejects_invalid_stage_counts() {
    use kb_component_runtime::{Error, KeyboardPipeline};
    assert!(matches!(
        KeyboardPipeline::<0>::new(&[]),
        Err(Error::InvalidChain)
    ));
    assert!(matches!(
        KeyboardPipeline::<3>::new(&[TOUCAN; 3]),
        Err(Error::InvalidChain)
    ));
}

fn synthetic_component(output: &[u8]) -> KeyboardComponent {
    let data = output
        .iter()
        .map(|byte| format!("\\{byte:02x}"))
        .collect::<String>();
    let source = format!(
        r#"(module
        (memory (export "memory") 1)
        (func (export "cabi_realloc") (param i32 i32 i32 i32) (result i32) i32.const 1024)
        (func (export "kb:firmware/keyboard@0.1.0#process") (param i32 i32 i32 i32 i32) (result i32) i32.const 0)
        (func (export "kb:firmware/keyboard@0.1.0#transform-output") (param i32 i32 i32 i32) (result i32) i32.const 0)
        (func (export "cabi_post_kb:firmware/keyboard@0.1.0#process") (param i32))
        (func (export "cabi_post_kb:firmware/keyboard@0.1.0#transform-output") (param i32))
        (data (i32.const 0) "{data}"))"#
    );
    KeyboardComponent::new(&wat::parse_str(source).unwrap()).unwrap()
}

#[test]
fn wake_times_preserve_unsigned_values() {
    let mut bytes = [0; 32];
    bytes[24] = 1;
    bytes[28..32].copy_from_slice(&u32::MAX.to_le_bytes());
    let component = synthetic_component(&bytes);
    assert_eq!(
        component
            .process::<8, 8, 8>(&[], &Input::tick(0))
            .unwrap()
            .wake_at_ms,
        Some(u32::MAX)
    );
}

#[test]
fn excessive_results_are_rejected_without_reading_guest_memory() {
    let mut bytes = [0; 32];
    bytes[0..4].copy_from_slice(&u32::MAX.to_le_bytes());
    bytes[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    let component = synthetic_component(&bytes);
    assert!(matches!(
        component.process::<8, 8, 8>(&[], &Input::tick(0)),
        Err(kb_component_runtime::Error::Capacity)
    ));
}

#[test]
fn invalid_variant_tags_and_out_of_bounds_lists_are_rejected() {
    let mut bytes = [0; 40];
    bytes[24] = 2;
    assert!(matches!(
        synthetic_component(&bytes).process::<8, 8, 8>(&[], &Input::tick(0)),
        Err(kb_component_runtime::Error::InvalidReturn)
    ));
    bytes[24] = 0;
    bytes[8..12].copy_from_slice(&32u32.to_le_bytes());
    bytes[12..16].copy_from_slice(&1u32.to_le_bytes());
    bytes[34] = 2;
    assert!(matches!(
        synthetic_component(&bytes).process::<8, 8, 8>(&[], &Input::tick(0)),
        Err(kb_component_runtime::Error::InvalidReturn)
    ));
    bytes[8..12].copy_from_slice(&65535u32.to_le_bytes());
    assert!(matches!(
        synthetic_component(&bytes).process::<8, 8, 8>(&[], &Input::tick(0)),
        Err(kb_component_runtime::Error::InvalidReturn)
    ));
}
