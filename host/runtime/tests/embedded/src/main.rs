#![no_std]
#![no_main]

use core::{fmt::Write, panic::PanicInfo};
use cortex_m_rt::entry;
use kb_component_runtime::DisplayGuest;
use kb_display::{
    Scene,
    lvgl::{Profile, Strip, TFT_WIDTH},
};

fn host(mut op: u32, value: usize) {
    unsafe { core::arch::asm!("bkpt #0xab", inout("r0") op, in("r1") value) };
    let _ = op;
}

fn exit(code: usize) -> ! {
    host(0x20, [0x20026usize, code].as_ptr() as usize);
    loop {
        core::hint::spin_loop();
    }
}

struct Output;

impl Write for Output {
    fn write_str(&mut self, text: &str) -> core::fmt::Result {
        for chunk in text.as_bytes().chunks(127) {
            let mut output = [0u8; 128];
            output[..chunk.len()].copy_from_slice(chunk);
            host(0x04, output.as_ptr() as usize);
        }
        Ok(())
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let _ = writeln!(Output, "{info}");
    exit(1)
}

#[cfg(feature = "keyboard")]
fn typing() -> impl DisplayGuest {
    use kb_component_runtime::KeyboardComponent;
    use kb_core::pipeline::{Bounded, Input, KeyEvent, KeyState};

    let guest = KeyboardComponent::new_embedded().unwrap();
    let mut state = Bounded::<u8, 64>::default();
    for cycle in 0..8 {
        for state_change in [KeyState::Pressed, KeyState::Released] {
            let events = core::array::from_fn::<_, 42, _>(|position| KeyEvent {
                position: position as u16,
                state: state_change,
                timestamp_ms: cycle * 10,
            });
            let input = Input::with_events(cycle * 10, &events).unwrap();
            let output = guest
                .process::<64, 42, 42>(state.as_slice(), &input)
                .unwrap();
            let output = guest
                .transform_output::<64, 42>(output.state.as_slice(), output.actions.as_slice())
                .unwrap();
            state = output.state;
        }
    }
    guest
}

#[cfg(not(feature = "keyboard"))]
fn typing() -> impl DisplayGuest {
    use kb_component_runtime::{BrailleComponent, BrailleState};

    let mut guest = BrailleComponent::new_embedded().unwrap();
    for cell in 1..=511 {
        let pressed = guest.step(BrailleState::default(), cell).unwrap();
        guest.step(pressed.state, 0).unwrap();
    }
    guest
}

fn display(guest: &mut impl DisplayGuest) {
    let detail = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuv";
    for (profile, width, height) in [(Profile::Tft, 280, 240), (Profile::Mip, 144, 168)] {
        let mut largest = (0, 0, 0);
        let mut strip = Strip::<{ TFT_WIDTH * 4 }>::EMPTY;
        for screen in 0..=13 {
            let indices: &[u16] = match screen {
                2 => &[0, 1, 2, 3],
                3 => &[0, 1, 2, 3, 4, 5, 6, 7],
                4 | 5 => &[
                    0, 15, 24, 0x100, 0x10f, 0x200, 0x300, 0x400, 0x500, 0x600, 0x700,
                ],
                6 => &[0, 1, 2, 3],
                7 => &[0, 1, 2],
                8 => &[0, 3, 4, 11],
                10 => &[0, 5],
                13 => &[0, 3, 4, 9],
                _ => &[0],
            };
            for &index in indices {
                let [lo, hi] = index.to_le_bytes();
                let bytes = [
                    screen, lo, hi, 0xff, 1, 0xff, 1, 15, 66, 80, 4, 0, 0, 4, 4, 1, 0, 3, 0, 9, 0,
                    25, 1, 4, 33, 0, 41, 1, 45, 0, 49, 1,
                ];
                let data = guest
                    .render(width, height, &bytes, detail)
                    .unwrap_or_else(|error| {
                        panic!("display {width}x{height}, screen {screen}, item {index}: {error:?}")
                    });
                let scene = Scene::from_bytes(data.as_slice()).unwrap_or_else(|| {
                    panic!("invalid draw commands for screen {screen}, item {index}")
                });
                largest = largest.max((scene.as_bytes().len(), screen, index));
                for y in (0..usize::from(height)).step_by(4) {
                    assert!(strip.render(profile, &scene, y..y + 4).is_some());
                }
            }
        }
        let _ = writeln!(
            Output,
            "{width}x{height} rendered, largest scene {largest:?}"
        );
    }
}

unsafe extern "C" {
    fn kb_embedded_begin(fuel: u32) -> u32;
    fn kb_embedded_memory_len() -> usize;
    fn kb_embedded_call(operation: u32, arguments: *const u32, output: *mut u32) -> u32;
}

fn traps() {
    let max = if cfg!(feature = "keyboard") {
        131072
    } else {
        65536
    };
    let arguments = [0, 0, 1, max + 1, 0, 0];
    let mut output = 0;
    unsafe {
        assert_eq!(kb_embedded_begin(0), 0);
        assert_ne!(kb_embedded_call(0, arguments.as_ptr(), &mut output), 0);
        assert_eq!(kb_embedded_begin(100_000), 0);
        assert_ne!(kb_embedded_call(0, arguments.as_ptr(), &mut output), 0);
        if cfg!(feature = "keyboard") {
            let arguments = [0, 0, 1, 65537, 0, 0];
            assert_eq!(kb_embedded_begin(100_000), 0);
            assert_eq!(kb_embedded_call(0, arguments.as_ptr(), &mut output), 0);
            assert_eq!(kb_embedded_memory_len(), 131072);
        }
    }
}

#[entry]
fn run() -> ! {
    let mut guest = typing();
    display(&mut guest);
    drop(guest);
    traps();
    let _ = writeln!(
        Output,
        "Embedded typing, display, fuel, and memory checks passed"
    );
    exit(0)
}
