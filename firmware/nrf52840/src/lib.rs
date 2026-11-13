#![no_std]

#[cfg(all(
    feature = "keyboard-composed",
    any(
        feature = "composed",
        feature = "nice-view",
        feature = "prospector-display",
        feature = "display"
    )
))]
compile_error!("keyboard composition requires the no-display Toucan configuration");

use embassy_nrf::config::{Config, HfclkSource, LfclkSource};
use embedded_alloc::LlffHeap;

#[global_allocator]
static HEAP: LlffHeap = LlffHeap::empty();

pub fn init_heap() {
    unsafe {
        #[cfg(feature = "keyboard-composed")]
        embedded_alloc::init!(HEAP, 196 * 1024);
        #[cfg(all(feature = "composed", not(feature = "keyboard-composed")))]
        embedded_alloc::init!(HEAP, 208 * 1024);
        #[cfg(not(any(feature = "composed", feature = "keyboard-composed")))]
        embedded_alloc::init!(HEAP, 160 * 1024);
    }
}

pub fn clock_config() -> Config {
    let mut config = Config::default();
    config.hfclk_source = HfclkSource::ExternalXtal;
    config.lfclk_source = LfclkSource::ExternalXtal;
    config
}

#[cfg(feature = "display")]
#[path = "../../../paintbrush/firmware/display.rs"]
pub mod display;

#[cfg(feature = "ble")]
pub mod ble_bonds;

#[cfg(not(feature = "ble"))]
mod critical_section;

#[cfg(feature = "split")]
pub(crate) mod radio;

#[cfg(feature = "split")]
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

#[cfg(feature = "split")]
pub mod settings;
