#![no_std]

#[cfg(any(feature = "lvgl-tft", feature = "lvgl-mip"))]
pub mod lvgl;
pub mod scene;
#[cfg(feature = "lvgl-tft")]
pub mod st7789;
#[path = "../../../toucan/display/mip.rs"]
pub mod toucan_display;

pub use scene::Scene;
