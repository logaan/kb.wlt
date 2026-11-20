use crate::{BrailleComponent, BrailleState, Error, KeyboardComponent, Runtime};
use kb_client_session::{Guest, Viewport};
use kb_core::{KeyStroke, pipeline::Bounded};

pub trait DisplayGuest: Guest<Error = Error> {
    fn render(
        &mut self,
        width: u16,
        height: u16,
        model: &[u8],
        detail: &str,
    ) -> Result<Bounded<u8, 768>, Error>;
}

macro_rules! display_guest {
    ($ty:ty) => {
        impl Guest for $ty {
            type Error = Error;
            fn step(
                &mut self,
                held: u16,
                chord: u16,
                keys: u16,
            ) -> Result<(u16, u16, Option<KeyStroke>), Error> {
                let bindings = self.braille_bindings()?;
                let result = self.with_runtime(|runtime| {
                    bindings.step(runtime, BrailleState { held, chord }, keys)
                })?;
                Ok((result.state.held, result.state.chord, result.stroke()))
            }

            fn map_cell(&mut self, cell: u16) -> Result<u32, Error> {
                let bindings = self.braille_bindings()?;
                self.with_runtime(|runtime| bindings.map_cell(runtime, cell))
            }

            fn navigate(
                &mut self,
                screen: u8,
                selected: u16,
                cell: u16,
                view: Viewport,
            ) -> Result<u32, Error> {
                self.with_runtime(|runtime| runtime.navigate(screen, selected, cell, view))
            }
        }

        impl DisplayGuest for $ty {
            fn render(
                &mut self,
                width: u16,
                height: u16,
                model: &[u8],
                detail: &str,
            ) -> Result<Bounded<u8, 768>, Error> {
                self.with_runtime(|runtime| runtime.render(width, height, model, detail))
            }
        }
    };
}

impl BrailleComponent {
    fn with_runtime<T>(
        &mut self,
        f: impl FnOnce(&mut Runtime) -> Result<T, Error>,
    ) -> Result<T, Error> {
        f(&mut self.runtime)
    }

    fn braille_bindings(&self) -> Result<crate::BrailleBindings, Error> {
        Ok(self.bindings)
    }
}

display_guest!(BrailleComponent);
display_guest!(KeyboardComponent);

impl Runtime {
    fn navigate(
        &mut self,
        screen: u8,
        selected: u16,
        cell: u16,
        view: Viewport,
    ) -> Result<u32, Error> {
        self.set_fuel(crate::CALL_FUEL)?;
        let navigate =
            self.func::<(i32, i32, i32, i32, i32, i32), i32>("kb:firmware/display@0.1.0#navigate")?;
        let value = navigate.call(
            self,
            (
                screen.into(),
                selected.into(),
                cell.into(),
                view.width.into(),
                view.height.into(),
                i32::from(view.host_switch),
            ),
        )?;
        self.func::<i32, ()>("cabi_post_kb:firmware/display@0.1.0#navigate")?
            .call(self, value)?;
        Ok(value as u32)
    }

    fn render(
        &mut self,
        width: u16,
        height: u16,
        model: &[u8],
        detail: &str,
    ) -> Result<Bounded<u8, 768>, Error> {
        self.set_fuel(crate::CALL_FUEL * 12)?;
        let realloc = self.func::<(i32, i32, i32, i32), i32>("cabi_realloc")?;
        let mut input = |bytes: &[u8]| -> Result<(i32, i32), Error> {
            if bytes.is_empty() {
                return Ok((0, 0));
            }
            let len = bytes.len().try_into().map_err(|_| Error::Capacity)?;
            let ptr = realloc.call(self, (0, 0, 1, len))?;
            self.write(ptr as usize, bytes)?;
            Ok((ptr, len))
        };
        let (model_ptr, model_len) = input(model)?;
        let (label_ptr, label_len) = input(detail.as_bytes())?;
        let render =
            self.func::<(i32, i32, i32, i32, i32, i32), i32>("kb:firmware/display@0.1.0#render")?;
        let ptr = render.call(
            self,
            (
                width.into(),
                height.into(),
                model_ptr,
                model_len,
                label_ptr,
                label_len,
            ),
        )?;
        let result = self.read_scene(ptr);
        self.func::<i32, ()>("cabi_post_kb:firmware/display@0.1.0#render")?
            .call(self, ptr)?;
        result
    }

    fn read_scene(&self, ptr: i32) -> Result<Bounded<u8, 768>, Error> {
        let memory = self.memory();
        let pair = |descriptor: &[u8], stride: usize| -> Result<&[u8], Error> {
            let offset = u32::from_le_bytes(descriptor[..4].try_into().unwrap()) as usize;
            let len = u32::from_le_bytes(descriptor[4..8].try_into().unwrap()) as usize;
            let end = len
                .checked_mul(stride)
                .and_then(|n| offset.checked_add(n))
                .ok_or(Error::InvalidReturn)?;
            memory.get(offset..end).ok_or(Error::InvalidReturn)
        };
        let descriptor = self.read::<16>(ptr)?;
        let commands = pair(&descriptor[..8], 14)?;
        let labels = pair(&descriptor[8..], 8)?;
        if commands.len() / 14 > 96 || labels.len() / 8 > 96 {
            return Err(Error::InvalidReturn);
        }
        let mut output = Bounded::default();
        for command in commands.chunks_exact(14) {
            output.push(command[0]).map_err(|_| Error::Capacity)?;
            output
                .extend_from_slice(&command[2..6])
                .map_err(|_| Error::Capacity)?;
            if command[0] == 3 {
                let i = usize::from(command[12]) * 8;
                let text = pair(labels.get(i..i + 8).ok_or(Error::InvalidReturn)?, 1)?;
                let len = text.len().try_into().map_err(|_| Error::Capacity)?;
                output
                    .extend_from_slice(&command[10..12])
                    .map_err(|_| Error::Capacity)?;
                output.push(len).map_err(|_| Error::Capacity)?;
                output
                    .extend_from_slice(text)
                    .map_err(|_| Error::Capacity)?;
            } else if matches!(command[0], 1 | 2 | 4..=6) {
                let end = if matches!(command[0], 4 | 6) { 13 } else { 12 };
                output
                    .extend_from_slice(&command[6..end])
                    .map_err(|_| Error::Capacity)?;
            } else {
                return Err(Error::InvalidReturn);
            }
        }
        Ok(output)
    }
}
