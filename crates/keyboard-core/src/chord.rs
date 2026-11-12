pub const SPACE_DOT: u16 = 0x100;
pub const CHORD_MASK: u16 = 0x1ff;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PhysicalKey {
    pub source: u8,
    pub position: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DotEvent {
    pub key: PhysicalKey,
    pub bit: u16,
    pub pressed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChordError {
    InvalidDot,
    BindingChanged,
    Capacity,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct HeldKey {
    key: PhysicalKey,
    bit: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChordState<const N: usize> {
    held: [HeldKey; N],
    accumulated: u16,
    discarded: bool,
}

impl<const N: usize> Default for ChordState<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> ChordState<N> {
    pub const fn new() -> Self {
        Self {
            held: [HeldKey {
                key: PhysicalKey {
                    source: 0,
                    position: 0,
                },
                bit: 0,
            }; N],
            accumulated: 0,
            discarded: false,
        }
    }

    pub fn held_mask(&self) -> u16 {
        if self.discarded {
            return 0;
        }
        self.held.iter().fold(0, |mask, key| mask | key.bit)
    }

    pub fn is_idle(&self) -> bool {
        self.held.iter().all(|key| key.bit == 0)
    }

    pub fn discard(mut self) -> Self {
        self.discarded = !self.is_idle();
        self.accumulated = 0;
        self
    }

    pub fn disconnect(mut self, source: u8) -> Self {
        let affected = self
            .held
            .iter()
            .any(|held| held.bit != 0 && held.key.source == source);
        if !affected {
            return self;
        }
        for held in &mut self.held {
            if held.key.source == source {
                held.bit = 0;
            }
        }
        self.accumulated = 0;
        self.discarded = !self.is_idle();
        self
    }

    pub fn transition(mut self, event: DotEvent) -> Result<(Self, Option<u16>), ChordError> {
        if event.bit == 0 || event.bit & !CHORD_MASK != 0 || !event.bit.is_power_of_two() {
            return Err(ChordError::InvalidDot);
        }
        if let Some(index) = self
            .held
            .iter()
            .position(|held| held.bit != 0 && held.key == event.key)
        {
            if self.held[index].bit != event.bit {
                return Err(ChordError::BindingChanged);
            }
            if event.pressed {
                return Ok((self, None));
            }
            self.held[index].bit = 0;
            if !self.is_idle() {
                return Ok((self, None));
            }
            let completed = if self.discarded {
                None
            } else {
                Some(self.accumulated)
            };
            self.accumulated = 0;
            self.discarded = false;
            return Ok((self, completed));
        }
        if !event.pressed {
            return Ok((self, None));
        }
        let free = self
            .held
            .iter_mut()
            .find(|held| held.bit == 0)
            .ok_or(ChordError::Capacity)?;
        *free = HeldKey {
            key: event.key,
            bit: event.bit,
        };
        self.accumulated |= event.bit;
        Ok((self, None))
    }
}
