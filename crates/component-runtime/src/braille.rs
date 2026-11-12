use crate::{BrailleComponent, BrailleState, Error};
use kb_core::{
    KeyStroke,
    preferences::{Engine, Fallback, Output as StrokeBatch, Settings},
    transport::FaultLatch,
};

pub struct BrailleTyping {
    component: BrailleState,
    engine: Engine,
    buffer_until_release: bool,
    held: u16,
    chord: u16,
    tentative: Fallback,
}

impl BrailleTyping {
    pub fn new(buffer_until_release: bool) -> Self {
        Self {
            component: BrailleState::default(),
            engine: Engine::default(),
            buffer_until_release,
            held: 0,
            chord: 0,
            tentative: Fallback::default(),
        }
    }

    pub fn cancel(&mut self) {
        *self = Self::new(self.buffer_until_release);
    }

    pub fn held(&self) -> u16 {
        self.held
    }

    pub fn chord(&self) -> u16 {
        self.chord
    }

    pub fn step(
        &mut self,
        component: &mut FaultLatch<BrailleComponent>,
        settings: &Settings,
        keys: u16,
        now: u32,
    ) -> StrokeBatch {
        if component.is_faulted() {
            return StrokeBatch::default();
        }
        if keys == self.held {
            return self.engine.poll(settings, now);
        }
        self.held = keys;
        self.chord |= keys;
        let result = component.call(|component| -> Result<_, Error> {
            let output = component.step(self.component, keys)?;
            self.component = output.state;
            let fallback =
                (output.usage != 0).then_some(KeyStroke::new(output.usage, output.modifiers));
            if self.buffer_until_release {
                if let Some(stroke) = fallback {
                    self.tentative.push(stroke).map_err(|_| Error::Capacity)?;
                }
                Ok(None)
            } else {
                Ok(fallback)
            }
        });
        match result {
            Ok(fallback) if keys == 0 => {
                let cell = core::mem::take(&mut self.chord);
                if self.buffer_until_release {
                    self.engine.accept_batch(
                        settings,
                        cell,
                        core::mem::take(&mut self.tentative),
                        now,
                    )
                } else {
                    self.engine.accept(settings, cell, fallback, now)
                }
            }
            Ok(Some(stroke)) => StrokeBatch::from_slice(&[stroke], KeyStroke::default()).unwrap(),
            Ok(None) => self.engine.poll(settings, now),
            Err(_) => {
                self.cancel();
                StrokeBatch::default()
            }
        }
    }
}
