use crate::{BrailleComponent, BrailleState, Error};
use kb_core::{
    chord::{CHORD_MASK, RecentChord},
    preferences::{Engine, Fallback, Output as StrokeBatch, Settings},
    transport::FaultLatch,
};

pub struct BrailleTyping {
    component: BrailleState,
    engine: Engine,
    buffer_until_release: bool,
    capture: RecentChord,
    chord: u16,
    tentative: Fallback,
}

impl BrailleTyping {
    pub fn new(buffer_until_release: bool) -> Self {
        Self {
            component: BrailleState::default(),
            engine: Engine::default(),
            buffer_until_release,
            capture: RecentChord::new(),
            chord: 0,
            tentative: Fallback::default(),
        }
    }

    pub fn cancel(&mut self) {
        *self = Self::new(self.buffer_until_release);
    }

    pub fn held(&self) -> u16 {
        self.capture.held()
    }

    pub fn chord(&self) -> u16 {
        self.chord
    }

    pub fn buffered_cells(&self) -> [u16; kb_core::preferences::MAX_TRIGGER + 3] {
        self.engine.buffered_cells()
    }

    pub fn preview_release(
        &self,
        settings: &Settings,
        mapped: Option<kb_core::KeyStroke>,
        now: u32,
    ) -> (StrokeBatch, bool) {
        let mut engine = self.engine;
        let mut batch = self.tentative;
        if let Some(stroke) = mapped {
            let _ = batch.push(stroke);
        }
        let output = engine.accept_batch(settings, self.chord, batch, now);
        (output, engine.buffered_cells()[0] != 0)
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
        let keys = keys & CHORD_MASK;
        let previous = self.capture.held();
        let candidate = self.capture.update(keys, now);
        let known = self.chord & candidate;
        if known != self.chord {
            self.tentative = Fallback::default();
        }
        self.chord = if keys == 0 { 0 } else { candidate };
        if keys == previous {
            return self.engine.poll(settings, now);
        }
        let result = component.call(|component| -> Result<_, Error> {
            let output = component.step(
                BrailleState {
                    chord: known,
                    ..self.component
                },
                keys,
            )?;
            self.component = output.state;
            let fallback = output.stroke();
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
                if self.buffer_until_release {
                    self.engine.accept_batch(
                        settings,
                        candidate,
                        core::mem::take(&mut self.tentative),
                        now,
                    )
                } else {
                    self.engine.accept(settings, candidate, fallback, now)
                }
            }
            Ok(Some(stroke)) => self.engine.accept_action(settings, stroke, now),
            Ok(None) => self.engine.poll(settings, now),
            Err(_) => {
                self.cancel();
                StrokeBatch::default()
            }
        }
    }
}
