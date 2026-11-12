#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_nrf::gpio::Level;
use embassy_nrf::pwm::{DutyCycle, Prescaler, SimpleConfig, SimplePwm};
use embassy_time::{Instant, Timer};
use kb_component_runtime::{LedComponent, LedState};
use kb_core::transport::FaultLatch;
use panic_halt as _;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    kb_nrf52840::init_heap();
    let p = embassy_nrf::init(kb_nrf52840::clock_config());
    let mut pwm_config = SimpleConfig::default();
    pwm_config.max_duty = 255;
    pwm_config.prescaler = Prescaler::Div64;
    pwm_config.ch0_idle_level = Level::High;
    pwm_config.ch1_idle_level = Level::High;
    pwm_config.ch2_idle_level = Level::High;
    let mut rgb = SimplePwm::new_3ch(p.PWM0, p.P0_26, p.P0_30, p.P0_06, &pwm_config);
    #[cfg(not(feature = "composed"))]
    let mut component = FaultLatch::new(LedComponent::new(include_bytes!(env!("KB_LED_WASM"))));
    #[cfg(feature = "composed")]
    let mut component = FaultLatch::new(LedComponent::new_chain(&[
        include_bytes!(env!("KB_LED_WRAPPER_WASM")),
        include_bytes!(env!("KB_LED_WASM")),
    ]));
    let mut state = LedState::default();
    loop {
        let now = Instant::now().as_millis() as u32;
        let colors = match component.call(|component| component.step(state, now, false)) {
            Ok(output) => {
                state = output.state;
                [output.red, output.green, output.blue]
            }
            Err(_) => [if now / 200 % 2 == 0 { 255 } else { 0 }, 0, 0],
        };
        rgb.set_all_duties([
            DutyCycle::normal(u16::from(colors[0])),
            DutyCycle::normal(u16::from(colors[1])),
            DutyCycle::normal(u16::from(colors[2])),
            DutyCycle::normal(0),
        ]);
        Timer::after_millis(10).await;
    }
}
