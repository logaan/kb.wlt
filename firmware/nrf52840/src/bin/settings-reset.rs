#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_nrf::gpio::{Level, Output, OutputDrive};
use embassy_nrf::nvmc::Nvmc;
use embassy_time::Timer;
use embedded_storage::nor_flash::{NorFlash, ReadNorFlash};
use panic_halt as _;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_nrf::init(kb_nrf52840::clock_config());
    let mut nano_led = Output::new(p.P0_15, Level::Low, OutputDrive::Standard);
    let mut xiao_led = Output::new(p.P0_30, Level::High, OutputDrive::Standard);
    let mut flash = Nvmc::new(p.NVMC);
    flash.erase(0xf2000, 0xf4000).unwrap();
    let mut check = [0; 64];
    for offset in (0xf2000..0xf4000).step_by(check.len()) {
        flash.read(offset, &mut check).unwrap();
        assert!(check.iter().all(|byte| *byte == 0xff));
    }
    loop {
        nano_led.set_high();
        xiao_led.set_low();
        Timer::after_millis(200).await;
        nano_led.set_low();
        xiao_led.set_high();
        Timer::after_millis(800).await;
    }
}
