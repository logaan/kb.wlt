use crate::{CALL_FUEL, Error};
use core::{
    marker::PhantomData,
    sync::atomic::{AtomicBool, Ordering},
};

static RUNNING: AtomicBool = AtomicBool::new(false);

unsafe extern "C" {
    fn kb_embedded_begin(fuel: u32) -> u32;
    fn kb_embedded_fuel(fuel: u32);
    fn kb_embedded_memory() -> *mut u8;
    fn kb_embedded_memory_len() -> usize;
    fn kb_embedded_call(operation: u32, arguments: *const u32, output: *mut u32) -> u32;
}

pub(crate) struct Runtime {
    faulted: bool,
}

impl Runtime {
    pub(crate) fn new() -> Result<Self, Error> {
        RUNNING
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .map_err(|_| Error::InvalidState)?;
        let runtime = Self { faulted: false };
        let status = unsafe { kb_embedded_begin(CALL_FUEL as u32) };
        if status == 0 {
            Ok(runtime)
        } else {
            Err(Error::Wasm(status))
        }
    }

    pub(crate) fn func<P: Params, R: ResultType>(
        &self,
        name: &str,
    ) -> Result<TypedFunc<P, R>, Error> {
        let (operation, param_count, result_count) = match name {
            "cabi_realloc" => (0, 4, 1),
            "kb:firmware/braille@0.1.0#map-cell" => (1, 1, 1),
            "kb:firmware/braille@0.1.0#step" => (2, 3, 1),
            "kb:firmware/display@0.1.0#render" => (3, 6, 1),
            "kb:firmware/display@0.1.0#navigate" => (4, 6, 1),
            "kb:firmware/keyboard@0.1.0#process" => (5, 5, 1),
            "kb:firmware/keyboard@0.1.0#transform-output" => (6, 4, 1),
            "cabi_post_kb:firmware/braille@0.1.0#map-cell" => (7, 1, 0),
            "cabi_post_kb:firmware/braille@0.1.0#step" => (8, 1, 0),
            "cabi_post_kb:firmware/display@0.1.0#render" => (9, 1, 0),
            "cabi_post_kb:firmware/display@0.1.0#navigate" => (10, 1, 0),
            "cabi_post_kb:firmware/keyboard@0.1.0#process" => (11, 1, 0),
            "cabi_post_kb:firmware/keyboard@0.1.0#transform-output" => (12, 1, 0),
            _ => return Err(Error::InvalidState),
        };
        if P::COUNT != param_count || R::COUNT != result_count {
            return Err(Error::InvalidState);
        }
        Ok(TypedFunc {
            operation,
            signature: PhantomData,
        })
    }

    pub(crate) fn set_fuel(&mut self, fuel: u64) -> Result<(), Error> {
        self.check()?;
        let fuel = fuel.try_into().map_err(|_| Error::Capacity)?;
        unsafe { kb_embedded_fuel(fuel) };
        Ok(())
    }

    fn check(&self) -> Result<(), Error> {
        if self.faulted {
            Err(Error::InvalidState)
        } else {
            Ok(())
        }
    }

    pub(crate) fn memory(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(kb_embedded_memory(), kb_embedded_memory_len()) }
    }

    pub(crate) fn write(&mut self, offset: usize, bytes: &[u8]) -> Result<(), Error> {
        self.check()?;
        let end = offset
            .checked_add(bytes.len())
            .ok_or(Error::InvalidReturn)?;
        let memory = unsafe {
            core::slice::from_raw_parts_mut(kb_embedded_memory(), kb_embedded_memory_len())
        };
        memory
            .get_mut(offset..end)
            .ok_or(Error::InvalidReturn)?
            .copy_from_slice(bytes);
        Ok(())
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        RUNNING.store(false, Ordering::Release);
    }
}

#[derive(Clone, Copy)]
pub(crate) struct TypedFunc<P, R> {
    operation: u32,
    signature: PhantomData<(P, R)>,
}

impl<P: Params, R: ResultType> TypedFunc<P, R> {
    pub(crate) fn call(self, runtime: &mut Runtime, params: P) -> Result<R, Error> {
        runtime.check()?;
        let mut output = 0;
        let params = params.padded();
        let status = unsafe { kb_embedded_call(self.operation, params.as_ptr(), &mut output) };
        if status != 0 {
            runtime.faulted = true;
            return Err(Error::Wasm(status));
        }
        Ok(R::from_word(output))
    }
}

pub(crate) trait Params {
    const COUNT: usize;
    fn padded(self) -> [u32; 6];
}

impl Params for i32 {
    const COUNT: usize = 1;

    fn padded(self) -> [u32; 6] {
        [self as u32, 0, 0, 0, 0, 0]
    }
}

macro_rules! tuple_params {
    ($length:expr, $($index:tt),+) => {
        impl Params for ($(tuple_params!(@type $index)),+) {
            const COUNT: usize = $length;

            fn padded(self) -> [u32; 6] {
                let mut args = [0; 6];
                $(args[$index] = self.$index as u32;)+
                args
            }
        }
    };
    (@type $index:tt) => { i32 };
}

tuple_params!(3, 0, 1, 2);
tuple_params!(4, 0, 1, 2, 3);
tuple_params!(5, 0, 1, 2, 3, 4);
tuple_params!(6, 0, 1, 2, 3, 4, 5);

pub(crate) trait ResultType {
    const COUNT: usize;
    fn from_word(value: u32) -> Self;
}

impl ResultType for i32 {
    const COUNT: usize = 1;

    fn from_word(value: u32) -> Self {
        value as i32
    }
}

impl ResultType for () {
    const COUNT: usize = 0;

    fn from_word(_: u32) -> Self {}
}
