use crate::{KeyStroke, pipeline::Bounded, reference::LEADER};

pub const BINDING_COUNT: usize = 12;
pub const MAX_TRIGGER: usize = 4;
pub const MAX_TEXT: usize = 24;
pub const ENCODED_LEN: usize = 6 + BINDING_COUNT * (2 + MAX_TRIGGER * 2 + MAX_TEXT);
pub const PREFIX_TIMEOUT_MS: u32 = 1_200;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum BrailleMode {
    #[default]
    Computer8,
    Ueb1,
}

impl BrailleMode {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Computer8 => "Computer 8",
            Self::Ueb1 => "UEB Grade 1",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Binding {
    pub trigger: [u16; MAX_TRIGGER],
    pub trigger_len: u8,
    pub text: [u8; MAX_TEXT],
    pub text_len: u8,
}

impl Binding {
    pub fn trigger(&self) -> &[u16] {
        &self.trigger[..usize::from(self.trigger_len).min(MAX_TRIGGER)]
    }

    pub fn text(&self) -> &[u8] {
        &self.text[..usize::from(self.text_len).min(MAX_TEXT)]
    }

    pub const fn is_empty(&self) -> bool {
        self.trigger_len == 0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    pub mode: BrailleMode,
    pub bindings: [Binding; BINDING_COUNT],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidLength,
    InvalidCell,
    InvalidText,
    ReservedChord,
    DuplicateTrigger,
    InvalidEncoding,
}

impl Settings {
    pub fn set_binding(&mut self, index: usize, trigger: &[u16], text: &[u8]) -> Result<(), Error> {
        if index >= BINDING_COUNT
            || trigger.is_empty()
            || trigger.len() > MAX_TRIGGER
            || text.is_empty()
            || text.len() > MAX_TEXT
        {
            return Err(Error::InvalidLength);
        }
        if trigger.iter().any(|cell| *cell == 0 || *cell > 0x1ff) {
            return Err(Error::InvalidCell);
        }
        if trigger.contains(&LEADER) {
            return Err(Error::ReservedChord);
        }
        if text
            .iter()
            .any(|byte| !(32..=126).contains(byte) && !matches!(byte, b'\n' | b'\t'))
        {
            return Err(Error::InvalidText);
        }
        if self
            .bindings
            .iter()
            .enumerate()
            .any(|(slot, binding)| slot != index && binding.trigger() == trigger)
        {
            return Err(Error::DuplicateTrigger);
        }
        let mut binding = Binding::default();
        binding.trigger[..trigger.len()].copy_from_slice(trigger);
        binding.trigger_len = trigger.len() as u8;
        binding.text[..text.len()].copy_from_slice(text);
        binding.text_len = text.len() as u8;
        self.bindings[index] = binding;
        Ok(())
    }

    pub fn clear_binding(&mut self, index: usize) -> Result<(), Error> {
        *self.bindings.get_mut(index).ok_or(Error::InvalidLength)? = Binding::default();
        Ok(())
    }

    pub fn encode(&self, output: &mut [u8]) -> Result<usize, Error> {
        if output.len() < ENCODED_LEN {
            return Err(Error::InvalidLength);
        }
        let mut validated = Self {
            mode: self.mode,
            ..Self::default()
        };
        for (index, binding) in self.bindings.iter().enumerate() {
            if binding.trigger_len as usize > MAX_TRIGGER || binding.text_len as usize > MAX_TEXT {
                return Err(Error::InvalidLength);
            }
            if binding.is_empty() {
                if binding.text_len != 0 {
                    return Err(Error::InvalidLength);
                }
            } else {
                validated.set_binding(index, binding.trigger(), binding.text())?;
            }
        }
        output[..ENCODED_LEN].fill(0);
        output[..4].copy_from_slice(b"KBQF");
        output[4] = 1;
        output[5] = self.mode as u8;
        for (index, binding) in validated.bindings.iter().enumerate() {
            let offset = 6 + index * 34;
            output[offset] = binding.trigger_len;
            output[offset + 1] = binding.text_len;
            for (dot, cell) in binding.trigger.iter().enumerate() {
                output[offset + 2 + dot * 2..offset + 4 + dot * 2]
                    .copy_from_slice(&cell.to_le_bytes());
            }
            output[offset + 10..offset + 34].copy_from_slice(&binding.text);
        }
        Ok(ENCODED_LEN)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() != ENCODED_LEN || &bytes[..4] != b"KBQF" || bytes[4] != 1 {
            return Err(Error::InvalidEncoding);
        }
        let mode = match bytes[5] {
            0 => BrailleMode::Computer8,
            1 => BrailleMode::Ueb1,
            _ => return Err(Error::InvalidEncoding),
        };
        let mut settings = Self {
            mode,
            ..Self::default()
        };
        for index in 0..BINDING_COUNT {
            let offset = 6 + index * 34;
            let trigger_len = bytes[offset] as usize;
            let text_len = bytes[offset + 1] as usize;
            if trigger_len > MAX_TRIGGER
                || text_len > MAX_TEXT
                || (trigger_len == 0) != (text_len == 0)
            {
                return Err(Error::InvalidLength);
            }
            let mut trigger = [0; MAX_TRIGGER];
            for (dot, cell) in trigger.iter_mut().enumerate() {
                *cell =
                    u16::from_le_bytes([bytes[offset + 2 + dot * 2], bytes[offset + 3 + dot * 2]]);
            }
            if trigger[trigger_len..].iter().any(|cell| *cell != 0)
                || bytes[offset + 10 + text_len..offset + 34]
                    .iter()
                    .any(|byte| *byte != 0)
            {
                return Err(Error::InvalidEncoding);
            }
            if trigger_len != 0 {
                settings.set_binding(
                    index,
                    &trigger[..trigger_len],
                    &bytes[offset + 10..offset + 10 + text_len],
                )?;
            }
        }
        Ok(settings)
    }
}

pub type Output = Bounded<KeyStroke, 64>;

#[derive(Clone, Copy, Debug, Default)]
struct Pending {
    cell: u16,
    fallback: Option<KeyStroke>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Engine {
    mode: BrailleMode,
    ueb: crate::ueb1::State,
    pending: [Pending; MAX_TRIGGER],
    len: usize,
    updated_ms: u32,
}

impl Engine {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn pending_len(&self) -> usize {
        self.len
    }

    pub fn accept(
        &mut self,
        settings: &Settings,
        cell: u16,
        fallback: Option<KeyStroke>,
        now_ms: u32,
    ) -> Output {
        let mut output = self.poll(settings, now_ms);
        if cell == 0 || cell > 0x1ff {
            return output;
        }
        self.pending[self.len] = Pending { cell, fallback };
        self.len += 1;
        self.updated_ms = now_ms;
        self.resolve(settings, false, &mut output);
        output
    }

    pub fn poll(&mut self, settings: &Settings, now_ms: u32) -> Output {
        if self.mode != settings.mode {
            self.reset();
            self.mode = settings.mode;
        }
        let mut output = Output::default();
        if self.len != 0 && now_ms.wrapping_sub(self.updated_ms) >= PREFIX_TIMEOUT_MS {
            self.resolve(settings, true, &mut output);
        }
        output
    }

    fn remove(&mut self, count: usize) {
        self.pending.copy_within(count..self.len, 0);
        self.len -= count;
    }

    fn resolve(&mut self, settings: &Settings, force: bool, output: &mut Output) {
        while self.len != 0 {
            let mut longest: Option<&Binding> = None;
            let mut longer = false;
            for binding in &settings.bindings {
                let trigger = binding.trigger();
                if trigger.is_empty() {
                    continue;
                }
                let common = trigger.len().min(self.len);
                if !trigger[..common]
                    .iter()
                    .zip(&self.pending[..common])
                    .all(|(cell, pending)| *cell == pending.cell)
                {
                    continue;
                }
                if trigger.len() > self.len {
                    longer = true;
                } else if longest.is_none_or(|current| trigger.len() > current.trigger().len()) {
                    longest = Some(binding);
                }
            }
            if longer && !force {
                break;
            }
            if let Some(binding) = longest {
                for byte in binding.text() {
                    if let Some(stroke) = KeyStroke::from_ascii(*byte) {
                        let _ = output.push(stroke);
                    }
                }
                self.ueb.after_text(binding.text());
                self.remove(binding.trigger().len());
            } else {
                let pending = self.pending[0];
                self.remove(1);
                match self.mode {
                    BrailleMode::Computer8 => {
                        if let Some(stroke) = pending.fallback {
                            let _ = output.push(stroke);
                        }
                    }
                    BrailleMode::Ueb1 => {
                        let _ = output.extend_from_slice(self.ueb.accept(pending.cell).as_slice());
                    }
                }
            }
        }
    }
}
