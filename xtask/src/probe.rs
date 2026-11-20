use kb_component_build::Contract;
use kb_component_runtime::{BrailleComponent, BrailleState, DisplayGuest, KeyboardComponent};
use kb_core::pipeline::{Input, KeyEvent, KeyState};
use std::{fs, path::PathBuf};

pub fn check(contract: Contract, paths: &[PathBuf], display: bool) -> Result<(), String> {
    let bytes = paths
        .iter()
        .map(fs::read)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let core = bytes.iter().map(Vec::as_slice).collect::<Vec<_>>();
    match contract {
        Contract::Braille => {
            let mut component = if core.len() == 1 {
                BrailleComponent::new(core[0])
            } else {
                BrailleComponent::new_chain(&core)
            }
            .map_err(|error| format!("braille runtime: {error:?}"))?;
            for keys in 1..512 {
                let state = component
                    .step(BrailleState::default(), keys)
                    .map_err(|error| format!("braille press: {error:?}"))?
                    .state;
                let _ = component
                    .step(state, 0)
                    .map_err(|error| format!("braille release: {error:?}"))?;
            }
            if display {
                probe_display(&mut component)?;
            }
        }
        Contract::Keyboard => {
            if core.len() != 1 {
                return Err("keyboard probe supports one component".into());
            }
            let mut component = KeyboardComponent::new(core[0])
                .map_err(|error| format!("keyboard runtime: {error:?}"))?;
            let mut state = Vec::new();
            for cycle in 0..8 {
                for pressed in [true, false] {
                    let events = (0..42)
                        .map(|position| KeyEvent {
                            position,
                            state: if pressed {
                                KeyState::Pressed
                            } else {
                                KeyState::Released
                            },
                            timestamp_ms: cycle * 10,
                        })
                        .collect::<Vec<_>>();
                    let input = Input::with_events(cycle * 10, &events)
                        .map_err(|error| format!("keyboard input: {error:?}"))?;
                    let output = component
                        .process::<64, 42, 42>(&state, &input)
                        .map_err(|error| format!("keyboard process: {error:?}"))?;
                    let output = component
                        .transform_output::<64, 42>(
                            output.state.as_slice(),
                            output.actions.as_slice(),
                        )
                        .map_err(|error| format!("keyboard output: {error:?}"))?;
                    state = output.state.as_slice().to_vec();
                }
            }
            if display {
                probe_display(&mut component)?;
            }
        }
    }
    Ok(())
}

fn probe_display(component: &mut impl DisplayGuest) -> Result<(), String> {
    for (width, height) in [(280, 240), (144, 168)] {
        for screen in 0..=13 {
            let selected: &[u16] = if matches!(screen, 4 | 5) {
                &[0, 15, 0x100, 0x200, 0x300, 0x400, 0x500, 0x600, 0x700]
            } else {
                &[0]
            };
            for selection in selected {
                let [lo, hi] = selection.to_le_bytes();
                let model = [
                    screen, lo, hi, 0xff, 1, 0xff, 1, 15, 81, 56, 4, 0, 0, 4, 4, 1, 0, 3, 0, 9, 0,
                    25, 1, 4, 33, 0, 41, 1, 45, 0, 49, 1,
                ];
                let scene = component
                    .render(
                        width,
                        height,
                        &model,
                        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuv",
                    )
                    .map_err(|error| {
                        format!(
                            "display {width}x{height}, screen {screen}, item {selection}: {error:?}"
                        )
                    })?;
                kb_display::Scene::from_bytes(scene.as_slice()).ok_or_else(|| {
                    format!("invalid drawing commands for screen {screen}, item {selection}")
                })?;
            }
        }
    }
    Ok(())
}
