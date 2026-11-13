#![no_std]

extern crate alloc;

mod braille;
mod keyboard;
mod link;

pub use braille::BrailleTyping;
pub use keyboard::{KeyboardComponent, KeyboardPipeline};

use wasmi::{
    Config, Engine, Instance, Linker, Memory, Module, Store, StoreLimits, StoreLimitsBuilder,
    TypedFunc,
};

pub const BRAILLE_EXPORT: &str = "kb:firmware/braille@0.1.0#step";
pub const MAX_MEMORY_BYTES: usize = 65_536;
pub const CALL_FUEL: u64 = 100_000;

#[derive(Debug)]
pub enum Error {
    Wasm(wasmi::Error),
    MissingMemory,
    InvalidReturn,
    InvalidState,
    InvalidChain,
    Capacity,
    Pipeline(kb_core::pipeline::PipelineError),
}

impl From<wasmi::Error> for Error {
    fn from(value: wasmi::Error) -> Self {
        Self::Wasm(value)
    }
}

struct Runtime {
    store: Store<StoreLimits>,
    instance: Instance,
    memory: Memory,
}

impl Runtime {
    fn new_chain(bytes: &[&[u8]]) -> Result<Self, Error> {
        if bytes.is_empty() || bytes.len() > 2 {
            return Err(Error::InvalidChain);
        }
        Self::with_engine(bytes, &Self::engine(false))
    }

    fn engine(eager: bool) -> Engine {
        let mut config = Config::default();
        config.consume_fuel(true);
        config.set_min_stack_height(1024);
        config.set_max_stack_height(16 * 1024);
        config.set_max_recursion_depth(64);
        config.set_max_cached_stacks(1);
        if eager {
            config.compilation_mode(wasmi::CompilationMode::Eager);
        }
        Engine::new(&config)
    }

    fn with_engine(bytes: &[&[u8]], engine: &Engine) -> Result<Self, Error> {
        let limits = StoreLimitsBuilder::new()
            .memory_size(MAX_MEMORY_BYTES)
            .table_elements(128)
            .instances(bytes.len())
            .memories(bytes.len())
            .tables(bytes.len())
            .trap_on_grow_failure(true)
            .build();
        let mut store = Store::new(engine, limits);
        store.limiter(|state| state);
        store.set_fuel(CALL_FUEL)?;
        let mut downstream = None;
        for bytes in bytes.iter().rev() {
            let module = Module::new(engine, bytes)?;
            let mut linker = Linker::new(engine);
            if let Some(provider) = downstream {
                link::connect(&mut linker, &store, provider)?;
            }
            downstream = Some(linker.instantiate_and_start(&mut store, &module)?);
        }
        let instance = downstream.ok_or(Error::InvalidChain)?;
        let memory = instance
            .get_memory(&store, "memory")
            .ok_or(Error::MissingMemory)?;
        Ok(Self {
            store,
            instance,
            memory,
        })
    }

    fn read<const N: usize>(&self, pointer: i32) -> Result<[u8; N], Error> {
        let offset = u32::try_from(pointer).map_err(|_| Error::InvalidReturn)? as usize;
        let mut bytes = [0; N];
        self.memory
            .read(&self.store, offset, &mut bytes)
            .map_err(|_| Error::InvalidReturn)?;
        Ok(bytes)
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
    step: TypedFunc<(i32, i32, i32), i32>,
    post_return: TypedFunc<i32, ()>,
    map_cell: TypedFunc<i32, i32>,
    map_post_return: TypedFunc<i32, ()>,
}

impl BrailleComponent {
    pub fn new(bytes: &[u8]) -> Result<Self, Error> {
        Self::new_chain(&[bytes])
    }

    pub fn new_chain(bytes: &[&[u8]]) -> Result<Self, Error> {
        let runtime = Runtime::new_chain(bytes)?;
        let step = runtime
            .instance
            .get_typed_func(&runtime.store, BRAILLE_EXPORT)?;
        let post_return = runtime
            .instance
            .get_typed_func(&runtime.store, "cabi_post_kb:firmware/braille@0.1.0#step")?;
        let map_cell = runtime
            .instance
            .get_typed_func(&runtime.store, "kb:firmware/braille@0.1.0#map-cell")?;
        let map_post_return = runtime.instance.get_typed_func(
            &runtime.store,
            "cabi_post_kb:firmware/braille@0.1.0#map-cell",
        )?;
        Ok(Self {
            runtime,
            step,
            post_return,
            map_cell,
            map_post_return,
        })
    }

    pub fn map_cell(&mut self, cell: u16) -> Result<u32, Error> {
        self.runtime.store.set_fuel(CALL_FUEL)?;
        let result = self
            .map_cell
            .call(&mut self.runtime.store, i32::from(cell))?;
        self.map_post_return.call(&mut self.runtime.store, result)?;
        Ok(result as u32)
    }

    pub fn step(&mut self, state: BrailleState, keys: u16) -> Result<BrailleOutput, Error> {
        self.runtime.store.set_fuel(CALL_FUEL)?;
        let pointer = self.step.call(
            &mut self.runtime.store,
            (state.held as i32, state.chord as i32, keys as i32),
        )?;
        let bytes = self.runtime.read::<10>(pointer);
        self.post_return.call(&mut self.runtime.store, pointer)?;
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
