#![no_std]

pub mod braille;
pub mod braille_hid;
pub mod chord;
pub mod debounce;
pub mod hid;
pub mod pipeline;
pub mod split;
pub mod storage;

pub use braille::computer8;
pub use chord::{ChordError, ChordState, DotEvent, PhysicalKey};
pub use debounce::{DebounceUpdate, Debouncer};
pub use hid::{KeyStroke, QueueFull, ReportQueue};

#[path = "../../../toucan/firmware/matrix_link.rs"]
pub mod matrix_link;
pub mod report_state;

pub mod transport;

pub mod menu;
pub mod preferences;
pub mod reference;
pub mod ueb1;
