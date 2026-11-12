use crate::pipeline::{Action, Key};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportError {
    Usage,
    Capacity,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Held {
    key: Key,
    active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reports {
    pub first: [u8; 8],
    pub second: Option<[u8; 8]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReportState<const N: usize> {
    held: [Held; N],
}

impl<const N: usize> Default for ReportState<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> ReportState<N> {
    pub const fn new() -> Self {
        Self {
            held: [Held {
                key: Key {
                    usage: 0,
                    modifiers: 0,
                },
                active: false,
            }; N],
        }
    }

    pub fn clear(&mut self) {
        *self = Self::new();
    }

    pub fn report(&self) -> [u8; 8] {
        let mut report = [0; 8];
        let mut count = 0;
        let mut rollover = false;
        for held in self.held.iter().filter(|held| held.active) {
            report[0] |= held.key.modifiers;
            if (224..=231).contains(&held.key.usage) {
                report[0] |= 1 << (held.key.usage - 224);
            } else if !report[2..2 + count].contains(&(held.key.usage as u8)) {
                if count < 6 {
                    report[count + 2] = held.key.usage as u8;
                    count += 1;
                } else {
                    rollover = true;
                }
            }
        }
        if rollover {
            report[2..].fill(1);
        }
        report
    }

    pub fn apply(&mut self, action: Action) -> Result<Reports, ReportError> {
        match action {
            Action::Press(key) => self.press(key)?,
            Action::Release(key) => self.release(key),
            Action::ReleaseAll => self.clear(),
            Action::Tap(key) => {
                let before = *self;
                self.press(key)?;
                let first = self.report();
                *self = before;
                return Ok(Reports {
                    first,
                    second: Some(self.report()),
                });
            }
        }
        Ok(Reports {
            first: self.report(),
            second: None,
        })
    }

    fn press(&mut self, key: Key) -> Result<(), ReportError> {
        if key.usage < 4 || key.usage > 255 {
            return Err(ReportError::Usage);
        }
        if self.held.iter().any(|held| held.active && held.key == key) {
            return Ok(());
        }
        let held = self
            .held
            .iter_mut()
            .find(|held| !held.active)
            .ok_or(ReportError::Capacity)?;
        *held = Held { key, active: true };
        Ok(())
    }

    fn release(&mut self, key: Key) {
        for held in &mut self.held {
            if held.active && held.key == key {
                held.active = false;
            }
        }
    }
}
