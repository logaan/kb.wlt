#![no_std]
#![no_main]

use core::convert::Infallible;
use embassy_executor::Spawner;
use embassy_nrf::gpio::{Level, Output, OutputDrive};
use embassy_time::Timer;
use kb_display::st7789::{PanelBus, St7789};
use panic_halt as _;

struct Bus {
    clock: Output<'static>,
    data: Output<'static>,
    cs: Output<'static>,
    dc: Output<'static>,
    reset: Output<'static>,
}

impl Bus {
    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            for bit in (0..8).rev() {
                self.data.set_level(Level::from(byte & (1 << bit) != 0));
                cortex_m::asm::delay(64);
                self.clock.set_high();
                cortex_m::asm::delay(64);
                self.clock.set_low();
            }
        }
    }
}

impl PanelBus for Bus {
    type Error = Infallible;

    async fn reset(&mut self) {
        self.cs.set_high();
        self.clock.set_low();
        self.reset.set_low();
        Timer::after_millis(20).await;
        self.reset.set_high();
        Timer::after_millis(120).await;
    }

    async fn delay_ms(&mut self, milliseconds: u32) {
        Timer::after_millis(u64::from(milliseconds)).await;
    }

    async fn command(&mut self, command: u8, data: &[u8]) -> Result<(), Self::Error> {
        self.cs.set_low();
        self.dc.set_low();
        self.write(&[command]);
        self.dc.set_high();
        self.write(data);
        self.cs.set_high();
        Ok(())
    }

    async fn data(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.cs.set_low();
        self.dc.set_high();
        self.write(data);
        self.cs.set_high();
        embassy_futures::yield_now().await;
        Ok(())
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_nrf::init(kb_nrf52840::clock_config());
    let mut led = Output::new(p.P0_26, Level::High, OutputDrive::Standard);
    let mut backlight = Output::new(p.P1_11, Level::Low, OutputDrive::Standard);
    let mut panel = St7789::new(Bus {
        clock: Output::new(p.P1_13, Level::Low, OutputDrive::HighDrive),
        data: Output::new(p.P1_15, Level::Low, OutputDrive::HighDrive),
        cs: Output::new(p.P1_14, Level::High, OutputDrive::Standard),
        dc: Output::new(p.P1_12, Level::High, OutputDrive::Standard),
        reset: Output::new(p.P0_29, Level::High, OutputDrive::Standard),
    });
    Timer::after_millis(250).await;
    panel.initialize().await.unwrap();
    backlight.set_high();
    loop {
        for color in [0xf800, 0x07e0, 0x001f, 0xffff] {
            led.set_low();
            panel.fill(color).await.unwrap();
            led.set_high();
            Timer::after_secs(2).await;
        }
    }
}
