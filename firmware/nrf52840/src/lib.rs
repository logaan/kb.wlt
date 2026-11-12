#![no_std]

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
