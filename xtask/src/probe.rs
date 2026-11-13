use kb_component_build::Contract;
use kb_component_runtime::{BrailleComponent, BrailleState, KeyboardComponent};
use kb_core::pipeline::{Input, KeyEvent, KeyState};
use std::{fs, path::PathBuf};

pub fn check(contract: Contract, paths: &[PathBuf], budget: usize) -> Result<usize, String> {
    let bytes = paths
        .iter()
        .map(fs::read)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let core = bytes.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let baseline = crate::heap::begin();
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
        }
        Contract::Keyboard => {
            if core.len() != 1 {
                return Err("keyboard probe supports one component".into());
            }
            let component = KeyboardComponent::new(core[0])
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
        }
    }
    let peak = crate::heap::peak_since(baseline);
    if peak > budget {
        return Err(format!(
            "component probe used {peak} heap bytes; target budget is {budget}. Rebuild Wavelet from the current checkout to omit unused Unicode tables, or simplify the component."
        ));
    }
    Ok(peak)
}
