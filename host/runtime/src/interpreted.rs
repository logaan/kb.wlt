use crate::{CALL_FUEL, Error, MAX_MEMORY_BYTES, link};
use alloc::vec::Vec;
use wasmi::{
    Config, Engine, Instance, Linker, Memory, Module, Store, StoreLimits, StoreLimitsBuilder,
    WasmParams, WasmResults,
};

#[derive(Clone, Copy)]
pub(crate) struct TypedFunc<P, R>(wasmi::TypedFunc<P, R>);

impl<P: WasmParams, R: WasmResults> TypedFunc<P, R> {
    pub(crate) fn call(self, runtime: &mut Runtime, params: P) -> Result<R, Error> {
        Ok(self.0.call(&mut runtime.store, params)?)
    }
}

impl From<wasmi::Error> for Error {
    fn from(value: wasmi::Error) -> Self {
        Self::Wasm(value)
    }
}

pub(crate) struct Runtime {
    store: Store<StoreLimits>,
    instance: Instance,
    memory: Memory,
}

impl Runtime {
    pub(crate) fn new_chain(bytes: &[&[u8]]) -> Result<Self, Error> {
        if bytes.is_empty() || bytes.len() > 2 {
            return Err(Error::InvalidChain);
        }
        Self::with_engine(bytes, &Self::engine(false))
    }

    pub(crate) fn engine(eager: bool) -> Engine {
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

    pub(crate) fn with_engine(bytes: &[&[u8]], engine: &Engine) -> Result<Self, Error> {
        let modules = bytes
            .iter()
            .rev()
            .map(|bytes| Module::new(engine, bytes))
            .collect::<Result<Vec<_>, _>>()?;
        let keyboard_display = matches!(modules.as_slice(), [module]
            if module.get_export("kb:firmware/display@0.1.0#render").is_some()
                && module.get_export("kb:firmware/keyboard@0.1.0#process").is_some()
        );
        let memory_limit = MAX_MEMORY_BYTES * if keyboard_display { 2 } else { 1 };
        let limits = StoreLimitsBuilder::new()
            .memory_size(memory_limit)
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
        for module in modules {
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

    pub(crate) fn func<P: WasmParams, R: WasmResults>(
        &self,
        name: &str,
    ) -> Result<TypedFunc<P, R>, Error> {
        Ok(TypedFunc(self.instance.get_typed_func(&self.store, name)?))
    }

    pub(crate) fn set_fuel(&mut self, fuel: u64) -> Result<(), Error> {
        Ok(self.store.set_fuel(fuel)?)
    }

    pub(crate) fn memory(&self) -> &[u8] {
        self.memory.data(&self.store)
    }

    pub(crate) fn write(&mut self, offset: usize, bytes: &[u8]) -> Result<(), Error> {
        self.memory
            .write(&mut self.store, offset, bytes)
            .map_err(|_| Error::InvalidReturn)
    }
}
