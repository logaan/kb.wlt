#![no_std]

pub mod braille;
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

pub mod matrix_link;
pub mod report_state;

pub mod transport;

pub mod reference;
