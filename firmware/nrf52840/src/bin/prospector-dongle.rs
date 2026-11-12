#![no_std]
#![no_main]

use embassy_executor::Spawner;
use panic_halt as _;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    kb_nrf52840::split::central(spawner, false).await;
}
