#![no_std]

#[cfg(feature = "lvgl")]
pub mod lvgl;
#[path = "../../../toucan/display/nice_view.rs"]
pub mod nice_view;
pub mod scene;
#[cfg(feature = "lvgl")]
pub mod st7789;

pub use scene::Scene;
