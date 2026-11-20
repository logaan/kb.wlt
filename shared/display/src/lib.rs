#![no_std]

#[cfg(any(feature = "lvgl-tft", feature = "lvgl-mip"))]
pub mod lvgl;
#[path = "../../../toucan/display/nice_view.rs"]
pub mod nice_view;
pub mod scene;
#[cfg(feature = "lvgl-tft")]
pub mod st7789;

pub use scene::Scene;
