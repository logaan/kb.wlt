use crate::{BRAILLE_EXPORT, Error, LED_EXPORT, LedStep};
use wasmi::{Caller, Instance, Linker, Memory, Store, StoreLimits, TypedFunc};

#[derive(Clone, Copy)]
pub(crate) enum Contract {
    Led,
    Braille,
}

pub(crate) fn connect(
    linker: &mut Linker<StoreLimits>,
    store: &Store<StoreLimits>,
    provider: Instance,
    contract: Contract,
) -> Result<(), Error> {
    let memory = provider
        .get_memory(store, "memory")
        .ok_or(Error::MissingMemory)?;
    match contract {
        Contract::Led => {
            let step: LedStep = provider.get_typed_func(store, LED_EXPORT)?;
            let post: TypedFunc<i32, ()> =
                provider.get_typed_func(store, "cabi_post_kb:firmware/led@0.1.0#step")?;
            linker
                .func_wrap(
                    "kb:firmware/led@0.1.0",
                    "step",
                    move |mut caller: Caller<'_, StoreLimits>,
                          phase: i32,
                          speed: i32,
                          last: i32,
                          button: i32,
                          now: i32,
                          down: i32,
                          output: i32|
                          -> Result<(), wasmi::Error> {
                        let pointer =
                            step.call(&mut caller, (phase, speed, last, button, now, down))?;
                        let result = transfer::<16>(&mut caller, memory, pointer, output);
                        post.call(&mut caller, pointer)?;
                        result
                    },
                )
                .map_err(wasmi::Error::from)?;
        }
        Contract::Braille => {
            let step: TypedFunc<(i32, i32, i32), i32> =
                provider.get_typed_func(store, BRAILLE_EXPORT)?;
            let post: TypedFunc<i32, ()> =
                provider.get_typed_func(store, "cabi_post_kb:firmware/braille@0.1.0#step")?;
            linker
                .func_wrap(
                    "kb:firmware/braille@0.1.0",
                    "step",
                    move |mut caller: Caller<'_, StoreLimits>,
                          held: i32,
                          chord: i32,
                          keys: i32,
                          output: i32|
                          -> Result<(), wasmi::Error> {
                        let pointer = step.call(&mut caller, (held, chord, keys))?;
                        let result = transfer::<10>(&mut caller, memory, pointer, output);
                        post.call(&mut caller, pointer)?;
                        result
                    },
                )
                .map_err(wasmi::Error::from)?;
            let map: TypedFunc<i32, i32> =
                provider.get_typed_func(store, "kb:firmware/braille@0.1.0#map-cell")?;
            let post: TypedFunc<i32, ()> =
                provider.get_typed_func(store, "cabi_post_kb:firmware/braille@0.1.0#map-cell")?;
            linker
                .func_wrap(
                    "kb:firmware/braille@0.1.0",
                    "map-cell",
                    move |mut caller: Caller<'_, StoreLimits>,
                          cell: i32|
                          -> Result<i32, wasmi::Error> {
                        let result = map.call(&mut caller, cell)?;
                        post.call(&mut caller, result)?;
                        Ok(result)
                    },
                )
                .map_err(wasmi::Error::from)?;
        }
    }
    Ok(())
}

fn transfer<const N: usize>(
    caller: &mut Caller<'_, StoreLimits>,
    source: Memory,
    pointer: i32,
    output: i32,
) -> Result<(), wasmi::Error> {
    let destination = caller
        .get_export("memory")
        .and_then(|export| export.into_memory())
        .ok_or_else(|| wasmi::Error::new("missing canonical destination memory"))?;
    let pointer = u32::try_from(pointer)
        .map_err(|_| wasmi::Error::new("invalid canonical result pointer"))?
        as usize;
    let output = u32::try_from(output)
        .map_err(|_| wasmi::Error::new("invalid canonical output pointer"))?
        as usize;
    let mut bytes = [0; N];
    source
        .read(&*caller, pointer, &mut bytes)
        .map_err(|_| wasmi::Error::new("canonical result is outside memory"))?;
    if N == 16 && bytes[8] > 1 {
        return Err(wasmi::Error::new("invalid canonical boolean result"));
    }
    destination
        .write(&mut *caller, output, &bytes)
        .map_err(|_| wasmi::Error::new("canonical output is outside memory"))?;
    Ok(())
}
