#![no_std]

extern crate alloc;

mod keyboard;
mod link;

pub use keyboard::KeyboardComponent;

use wasmi::{
    Config, Engine, Instance, Linker, Memory, Module, Store, StoreLimits, StoreLimitsBuilder,
    TypedFunc,
};

pub const LED_EXPORT: &str = "kb:firmware/led@0.1.0#step";
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
    fn new_chain(bytes: &[&[u8]], contract: link::Contract) -> Result<Self, Error> {
        if bytes.is_empty() || bytes.len() > 2 {
            return Err(Error::InvalidChain);
        }
        let mut config = Config::default();
        config.consume_fuel(true);
        config.set_min_stack_height(1024);
        config.set_max_stack_height(16 * 1024);
        config.set_max_recursion_depth(64);
        config.set_max_cached_stacks(1);
        let engine = Engine::new(&config);
        let limits = StoreLimitsBuilder::new()
            .memory_size(MAX_MEMORY_BYTES)
            .table_elements(128)
            .instances(bytes.len())
            .memories(bytes.len())
            .tables(bytes.len())
            .trap_on_grow_failure(true)
            .build();
        let mut store = Store::new(&engine, limits);
        store.limiter(|state| state);
        store.set_fuel(CALL_FUEL)?;
        let mut downstream = None;
        for bytes in bytes.iter().rev() {
            let module = Module::new(&engine, bytes)?;
            let mut linker = Linker::new(&engine);
            if let Some(provider) = downstream {
                link::connect(&mut linker, &store, provider, contract)?;
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
pub struct LedState {
    pub phase: u8,
    pub speed: u8,
    pub last_change_ms: u32,
    pub button_was_down: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LedOutput {
    pub state: LedState,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

type LedStep = TypedFunc<(i32, i32, i32, i32, i32, i32), i32>;

pub struct LedComponent {
    runtime: Runtime,
    step: LedStep,
    post_return: TypedFunc<i32, ()>,
}

impl LedComponent {
    pub fn new(bytes: &[u8]) -> Result<Self, Error> {
        Self::new_chain(&[bytes])
    }

    pub fn new_chain(bytes: &[&[u8]]) -> Result<Self, Error> {
        let runtime = Runtime::new_chain(bytes, link::Contract::Led)?;
        let step = runtime
            .instance
            .get_typed_func(&runtime.store, LED_EXPORT)?;
        let post_return = runtime
            .instance
            .get_typed_func(&runtime.store, "cabi_post_kb:firmware/led@0.1.0#step")?;
        Ok(Self {
            runtime,
            step,
            post_return,
        })
    }

    pub fn step(
        &mut self,
        state: LedState,
        now_ms: u32,
        button_down: bool,
    ) -> Result<LedOutput, Error> {
        self.runtime.store.set_fuel(CALL_FUEL)?;
        let pointer = self.step.call(
            &mut self.runtime.store,
            (
                state.phase as i32,
                state.speed as i32,
                state.last_change_ms as i32,
                state.button_was_down as i32,
                now_ms as i32,
                button_down as i32,
            ),
        )?;
        let bytes = self.runtime.read::<16>(pointer);
        self.post_return.call(&mut self.runtime.store, pointer)?;
        let bytes = bytes?;
        if bytes[8] > 1 {
            return Err(Error::InvalidReturn);
        }
        Ok(LedOutput {
            state: LedState {
                phase: bytes[0],
                speed: bytes[1],
                last_change_ms: u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
                button_was_down: bytes[8] == 1,
            },
            red: bytes[12],
            green: bytes[13],
            blue: bytes[14],
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BrailleState {
    pub held: u16,
    pub chord: u16,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BrailleOutput {
    pub state: BrailleState,
    pub usage: u8,
    pub modifiers: u8,
}

pub struct BrailleComponent {
    runtime: Runtime,
    step: TypedFunc<(i32, i32, i32), i32>,
    post_return: TypedFunc<i32, ()>,
}

impl BrailleComponent {
    pub fn new(bytes: &[u8]) -> Result<Self, Error> {
        Self::new_chain(&[bytes])
    }

    pub fn new_chain(bytes: &[&[u8]]) -> Result<Self, Error> {
        let runtime = Runtime::new_chain(bytes, link::Contract::Braille)?;
        let step = runtime
            .instance
            .get_typed_func(&runtime.store, BRAILLE_EXPORT)?;
        let post_return = runtime
            .instance
            .get_typed_func(&runtime.store, "cabi_post_kb:firmware/braille@0.1.0#step")?;
        Ok(Self {
            runtime,
            step,
            post_return,
        })
    }

    pub fn step(&mut self, state: BrailleState, keys: u16) -> Result<BrailleOutput, Error> {
        self.runtime.store.set_fuel(CALL_FUEL)?;
        let pointer = self.step.call(
            &mut self.runtime.store,
            (state.held as i32, state.chord as i32, keys as i32),
        )?;
        let bytes = self.runtime.read::<6>(pointer);
        self.post_return.call(&mut self.runtime.store, pointer)?;
        let bytes = bytes?;
        Ok(BrailleOutput {
            state: BrailleState {
                held: u16::from_le_bytes([bytes[0], bytes[1]]),
                chord: u16::from_le_bytes([bytes[2], bytes[3]]),
            },
            usage: bytes[4],
            modifiers: bytes[5],
        })
    }
}
