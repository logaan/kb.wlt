#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_nrf::gpio::{Level, Output, OutputDrive};
use embassy_time::{Instant, Timer};
use kb_component_runtime::{LedComponent, LedState};
use panic_halt as _;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    kb_nrf52840::init_heap();
    let p = embassy_nrf::init(kb_nrf52840::clock_config());
    let mut red = Output::new(p.P0_26, Level::High, OutputDrive::Standard);
    let mut green = Output::new(p.P0_30, Level::High, OutputDrive::Standard);
    let mut blue = Output::new(p.P0_06, Level::High, OutputDrive::Standard);
    let mut component = LedComponent::new(include_bytes!(
        "../../../../components/generated/led.core.wasm"
    ))
    .unwrap();
    let mut state = LedState::default();
    loop {
        let output = component
            .step(state, Instant::now().as_millis() as u32, false)
            .unwrap();
        state = output.state;
        red.set_level(if output.red > 0 {
            Level::Low
        } else {
            Level::High
        });
        green.set_level(if output.green > 0 {
            Level::Low
        } else {
            Level::High
        });
        blue.set_level(if output.blue > 0 {
            Level::Low
        } else {
            Level::High
        });
        Timer::after_millis(10).await;
    }
}
