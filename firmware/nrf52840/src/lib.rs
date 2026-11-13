#![no_std]

#[cfg(all(
    feature = "toucan-dongle",
    not(all(feature = "usb", feature = "prospector-display"))
))]
compile_error!("the Toucan receiver requires USB and the Prospector display");

#[cfg(all(
    feature = "toucan",
    not(any(
        feature = "toucan-dongle",
        feature = "toucan-right",
        feature = "nice-view"
    ))
))]
compile_error!("the left Toucan satellite requires its MIP display");

#[cfg(all(feature = "prospector-display", not(feature = "usb")))]
compile_error!("Prospector is a USB receiver");

#[cfg(all(
    feature = "split",
    not(feature = "toucan"),
    not(any(
        feature = "prospector-display",
        feature = "satellite-left",
        feature = "satellite-right"
    ))
))]
compile_error!(
    "Paintbrush requires a left satellite, right satellite, or Prospector receiver role"
);

#[cfg(all(feature = "satellite-left", feature = "satellite-right"))]
compile_error!("a Paintbrush satellite has exactly one side");

use embassy_nrf::config::{Config, HfclkSource, LfclkSource};
use embedded_alloc::LlffHeap;

#[global_allocator]
static HEAP: LlffHeap = LlffHeap::empty();

pub fn init_heap() {
    unsafe {
        embedded_alloc::init!(HEAP, 160 * 1024);
    }
}

pub fn clock_config() -> Config {
    let mut config = Config::default();
    config.hfclk_source = HfclkSource::ExternalXtal;
    config.lfclk_source = LfclkSource::ExternalXtal;
    config
}

#[cfg(feature = "ble")]
pub mod ble_bonds;

#[cfg(not(feature = "ble"))]
mod critical_section;

#[cfg(feature = "split")]
pub(crate) mod radio;

#[cfg(all(feature = "split", not(feature = "toucan")))]
#[path = "../../../paintbrush/firmware/split.rs"]
pub mod split;

#[cfg(feature = "prospector-display")]
pub mod prospector;

pub mod usb_hid;

#[cfg(feature = "nice-view")]
#[path = "../../../toucan/firmware/nice_view.rs"]
pub mod nice_view;

#[cfg(feature = "toucan")]
#[path = "../../../toucan/firmware/toucan.rs"]
pub mod toucan;

#[cfg(all(feature = "prospector-display", not(feature = "toucan")))]
pub mod settings;
