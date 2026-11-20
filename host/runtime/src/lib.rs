#![no_std]

#[cfg(not(target_os = "none"))]
extern crate alloc;

mod braille;
mod display;
pub use display::DisplayGuest;
mod keyboard;
#[cfg(not(target_os = "none"))]
mod link;

pub use braille::BrailleTyping;
pub use keyboard::KeyboardComponent;
#[cfg(not(target_os = "none"))]
pub use keyboard::KeyboardPipeline;

#[cfg(target_os = "none")]
mod embedded;
#[cfg(not(target_os = "none"))]
mod interpreted;

#[cfg(target_os = "none")]
use embedded::{Runtime, TypedFunc};
#[cfg(not(target_os = "none"))]
use interpreted::{Runtime, TypedFunc};

pub const BRAILLE_EXPORT: &str = "kb:firmware/braille@0.1.0#step";
pub const MAX_MEMORY_BYTES: usize = 65_536;
pub const CALL_FUEL: u64 = 100_000;

#[derive(Debug)]
pub enum Error {
    #[cfg(not(target_os = "none"))]
    Wasm(wasmi::Error),
    #[cfg(target_os = "none")]
    Wasm(u32),
    MissingMemory,
    InvalidReturn,
    InvalidState,
    InvalidChain,
    Capacity,
    Pipeline(kb_core::pipeline::PipelineError),
}

impl Runtime {
    fn read<const N: usize>(&self, pointer: i32) -> Result<[u8; N], Error> {
        let offset = u32::try_from(pointer).map_err(|_| Error::InvalidReturn)? as usize;
        let end = offset.checked_add(N).ok_or(Error::InvalidReturn)?;
        self.memory()
            .get(offset..end)
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or(Error::InvalidReturn)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BrailleState {
    pub held: u16,
    pub chord: u16,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BrailleActionKind {
    #[default]
    None,
    Tap,
    Sticky,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BrailleOutput {
    pub state: BrailleState,
    pub usage: u8,
    pub modifiers: u8,
    pub consumer: u16,
    pub kind: BrailleActionKind,
}

impl BrailleOutput {
    pub fn stroke(self) -> Option<kb_core::KeyStroke> {
        let stroke =
            kb_core::KeyStroke::new(self.usage, self.modifiers).with_consumer(self.consumer);
        match self.kind {
            BrailleActionKind::None => None,
            BrailleActionKind::Tap => Some(stroke),
            BrailleActionKind::Sticky => Some(stroke.sticky()),
        }
    }
}

pub struct BrailleComponent {
    runtime: Runtime,
    bindings: BrailleBindings,
}

#[derive(Clone, Copy)]
struct BrailleBindings {
    step: TypedFunc<(i32, i32, i32), i32>,
    post_return: TypedFunc<i32, ()>,
    map_cell: TypedFunc<i32, i32>,
    map_post_return: TypedFunc<i32, ()>,
}

impl BrailleComponent {
    #[cfg(not(target_os = "none"))]
    pub fn new(bytes: &[u8]) -> Result<Self, Error> {
        Self::new_chain(&[bytes])
    }

    #[cfg(not(target_os = "none"))]
    pub fn new_chain(bytes: &[&[u8]]) -> Result<Self, Error> {
        Self::from_runtime(Runtime::new_chain(bytes)?)
    }

    #[cfg(target_os = "none")]
    pub fn new_embedded() -> Result<Self, Error> {
        Self::from_runtime(Runtime::new()?)
    }

    fn from_runtime(runtime: Runtime) -> Result<Self, Error> {
        let bindings = BrailleBindings::new(&runtime)?;
        Ok(Self { runtime, bindings })
    }

    pub fn map_cell(&mut self, cell: u16) -> Result<u32, Error> {
        self.bindings.map_cell(&mut self.runtime, cell)
    }

    pub fn step(&mut self, state: BrailleState, keys: u16) -> Result<BrailleOutput, Error> {
        self.bindings.step(&mut self.runtime, state, keys)
    }
}

impl BrailleBindings {
    fn new(runtime: &Runtime) -> Result<Self, Error> {
        let step = runtime.func(BRAILLE_EXPORT)?;
        let post_return = runtime.func("cabi_post_kb:firmware/braille@0.1.0#step")?;
        let map_cell = runtime.func("kb:firmware/braille@0.1.0#map-cell")?;
        let map_post_return = runtime.func("cabi_post_kb:firmware/braille@0.1.0#map-cell")?;
        Ok(Self {
            step,
            post_return,
            map_cell,
            map_post_return,
        })
    }

    fn map_cell(&self, runtime: &mut Runtime, cell: u16) -> Result<u32, Error> {
        runtime.set_fuel(CALL_FUEL)?;
        let result = self.map_cell.call(runtime, i32::from(cell))?;
        self.map_post_return.call(runtime, result)?;
        Ok(result as u32)
    }

    fn step(
        &self,
        runtime: &mut Runtime,
        state: BrailleState,
        keys: u16,
    ) -> Result<BrailleOutput, Error> {
        runtime.set_fuel(CALL_FUEL)?;
        let pointer = self.step.call(
            runtime,
            (state.held as i32, state.chord as i32, keys as i32),
        )?;
        let bytes = runtime.read::<10>(pointer);
        self.post_return.call(runtime, pointer)?;
        let bytes = bytes?;
        let consumer = u16::from_le_bytes([bytes[6], bytes[7]]);
        let kind = match bytes[8] {
            0 if bytes[4..8] == [0; 4] => BrailleActionKind::None,
            1 | 2 => {
                let packed = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]])
                    | if bytes[8] == 2 { 1 << 31 } else { 0 };
                if consumer > 0x3fff || kb_core::KeyStroke::from_packed(packed).is_none() {
                    return Err(Error::InvalidReturn);
                }
                if bytes[8] == 2 {
                    BrailleActionKind::Sticky
                } else {
                    BrailleActionKind::Tap
                }
            }
            _ => return Err(Error::InvalidReturn),
        };
        Ok(BrailleOutput {
            state: BrailleState {
                held: u16::from_le_bytes([bytes[0], bytes[1]]),
                chord: u16::from_le_bytes([bytes[2], bytes[3]]),
            },
            usage: bytes[4],
            modifiers: bytes[5],
            consumer,
            kind,
        })
    }
}
