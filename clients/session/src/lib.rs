#![no_std]

pub mod labels;

use core::fmt::Write;
use kb_core::{
    KeyStroke,
    menu::{BluetoothAction, Effect, Notice, State},
    pipeline::Bounded,
    preferences::{BrailleMode, Engine, Fallback, Output, Settings},
    reference::{Action, LEADER, Mode},
};
use labels::{Label, escaped, release_label};

pub trait Guest {
    type Error;
    fn step(
        &mut self,
        held: u16,
        chord: u16,
        keys: u16,
    ) -> Result<(u16, u16, Option<KeyStroke>), Self::Error>;
    fn map_cell(&mut self, cell: u16) -> Result<u32, Self::Error>;
    fn navigate(
        &mut self,
        screen: u8,
        selected: u16,
        cell: u16,
        visible: u8,
    ) -> Result<u32, Self::Error>;
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Links {
    pub online: u8,
    pub battery: [u8; 2],
}

impl Default for Links {
    fn default() -> Self {
        Self {
            online: 0,
            battery: [255; 2],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Model {
    pub bytes: Bounded<u8, 48>,
    pub detail: Label,
}

impl Model {
    pub fn home(links: Links) -> Self {
        let mut model = [0; 16];
        model[7] = links.online;
        model[8..10].copy_from_slice(&links.battery);
        Self {
            bytes: Bounded::from_slice(&model, 0).unwrap(),
            detail: Label::new(),
        }
    }
}

#[derive(Default)]
pub struct Change {
    pub strokes: Output,
    pub save: bool,
    pub bluetooth: Option<BluetoothAction>,
    pub reset: bool,
}

pub struct Session {
    pub settings: Settings,
    screen: u8,
    selected: u16,
    help_return: (u8, u16),
    viewport: (u16, u16),
    host_switch: bool,
    menu: State,
    engine: Engine,
    held: u16,
    chord: u16,
    keys: u16,
    menu_dots: u16,
    space: bool,
    consumed: bool,
    guard: bool,
    fallback: Fallback,
    preview: Label,
}

impl Session {
    pub fn new(settings: Settings) -> Self {
        Self {
            settings,
            screen: 1,
            selected: 0,
            help_return: (1, 0),
            viewport: (280, 240),
            host_switch: false,
            menu: State::default(),
            engine: Engine::default(),
            held: 0,
            chord: 0,
            keys: 0,
            menu_dots: 0,
            space: false,
            consumed: false,
            guard: false,
            fallback: Fallback::default(),
            preview: Label::new(),
        }
    }

    pub fn set_viewport(&mut self, width: u16, height: u16) {
        self.viewport = (width.max(128), height.max(144));
    }

    pub fn enable_host_switch(&mut self) {
        self.host_switch = true;
    }

    pub fn cancel(&mut self, keys: u16) {
        let viewport = self.viewport;
        let host_switch = self.host_switch;
        *self = Self::new(self.settings);
        self.viewport = viewport;
        self.host_switch = host_switch;
        self.guard = keys != 0;
    }

    pub fn busy(&mut self) {
        self.menu.notice = Notice::Busy;
    }

    pub fn finish_momentary<G: Guest>(
        &mut self,
        final_keys: Option<u16>,
        now: u32,
        client: &mut G,
    ) -> Result<Change, G::Error> {
        let change = if final_keys == Some(0) {
            self.update(0, now, client)
        } else {
            Ok(Change::default())
        };
        self.cancel(0);
        change
    }

    fn chord_event(&mut self, keys: u16) -> Option<u16> {
        if self.keys == 0 && keys != 0 {
            self.consumed = false;
        }
        self.menu_dots |= keys & 255;
        self.space |= keys & 256 != 0;
        if self.menu_dots != 0 && keys & 255 == 0 {
            let event = core::mem::take(&mut self.menu_dots) | (u16::from(self.space) << 8);
            self.space = keys != 0;
            self.consumed = true;
            Some(event)
        } else if self.keys != 0 && keys == 0 {
            let event = (self.space && !self.consumed).then_some(256);
            self.space = false;
            self.consumed = false;
            event
        } else {
            None
        }
    }

    pub fn update<G: Guest>(
        &mut self,
        keys: u16,
        now: u32,
        client: &mut G,
    ) -> Result<Change, G::Error> {
        let keys = keys & 511;
        let mut change = Change::default();
        if self.guard {
            self.guard = keys != 0;
            return Ok(change);
        }
        let previous = self.keys;
        let cell = self.chord_event(keys);
        self.keys = keys;
        if self.screen == 1 && cell == Some(LEADER) {
            self.held = 0;
            self.chord = 0;
            self.engine.reset();
            self.fallback = Fallback::default();
            self.preview = Label::new();
            self.screen = 2;
            self.selected = 0;
            change.reset = true;
        } else if self.screen != 1 {
            if let Some(cell) = cell {
                self.navigate(cell, client, &mut change)?;
                if self.screen == 1 {
                    self.cancel(keys);
                }
            }
        } else if keys != previous {
            let (held, chord, stroke) = client.step(self.held, self.chord, keys)?;
            let released = self.chord;
            self.held = held;
            self.chord = chord;
            if let Some(stroke) = stroke {
                let _ = self.fallback.push(stroke);
            }
            if keys == 0 {
                change.strokes = self.engine.accept_batch(
                    &self.settings,
                    released,
                    core::mem::take(&mut self.fallback),
                    now,
                );
                self.preview = Label::new();
            } else {
                let mut preview_engine = self.engine;
                let mut batch = self.fallback;
                if let Some(stroke) = KeyStroke::from_packed(client.map_cell(chord)?) {
                    let _ = batch.push(stroke);
                }
                let result = preview_engine.accept_batch(&self.settings, chord, batch, now);
                self.preview = release_label(
                    result.as_slice(),
                    preview_engine.buffered_cells()[0] != 0,
                    chord == LEADER,
                );
            }
        } else {
            change.strokes = self.engine.poll(&self.settings, now);
        }
        Ok(change)
    }

    fn visible_rows(&self) -> u8 {
        if self.screen == 13 {
            return if self.host_switch { 5 } else { 4 };
        }
        let (width, height) = self.viewport;
        let wide = width >= 220;
        let footer = if wide { 34 } else { 25 };
        let header = if wide { 40 } else { 25 };
        let available = height.saturating_sub(header + footer + 6);
        let section = self.selected / 256;
        if matches!(self.screen, 4 | 5) {
            if section < 4 {
                return if wide { 16 } else { 12 };
            }
            let row_height = if wide {
                35
            } else if section >= 6 {
                44
            } else {
                24
            };
            let legend = if section >= 6 { footer } else { 0 };
            return (available.saturating_sub(legend) / row_height).clamp(1, 6) as u8;
        }
        let preview = if self.screen == 8 { 50 } else { 0 };
        let row_height = if wide { 34 } else { 23 };
        (available.saturating_sub(preview) / row_height).clamp(1, 8) as u8
    }

    fn navigate<G: Guest>(
        &mut self,
        cell: u16,
        client: &mut G,
        change: &mut Change,
    ) -> Result<(), G::Error> {
        let previous = (self.screen, self.selected);
        let visible = self.visible_rows();
        let packed = client.navigate(self.screen, self.selected, cell, visible)?;
        let action = (packed >> 24) as u8;
        self.screen = packed as u8;
        self.selected = (packed >> 8) as u16;
        if action == 4 && previous.0 != 13 {
            self.help_return = previous;
        }
        if previous.0 == 10 && self.screen == 8 && action == 2 {
            self.selected = u16::from(self.menu.slot);
        }
        if previous.0 == 13 && action == 2 {
            (self.screen, self.selected) = self.help_return;
            return Ok(());
        }
        let mode = match previous.0 {
            7 => Mode::Config,
            8 => Mode::Bindings,
            10 => Mode::BindingOptions,
            11 => Mode::BindingCapture,
            12 => Mode::BindingText,
            _ => return Ok(()),
        };
        let command = match action {
            1 => Action::Activate,
            2 if mode == Mode::BindingText => Action::Back,
            3 => Action::Lookup(cell),
            _ => return Ok(()),
        };
        self.menu.selected = previous.1 as u8;
        let stroke = if command == Action::Lookup(cell) {
            KeyStroke::from_packed(client.map_cell(cell)?)
        } else {
            None
        };
        let update = self.menu.step(mode, command, &self.settings, stroke);
        self.menu = update.state;
        self.selected = self.menu.selected.into();
        self.screen = match update.mode {
            Mode::Config => 7,
            Mode::Bindings => 8,
            Mode::BindingOptions => 10,
            Mode::BindingCapture => 11,
            Mode::BindingText => 12,
            _ => self.screen,
        };
        change.save = match update.effect {
            Effect::SetMode(mode) => {
                self.settings.mode = mode;
                true
            }
            Effect::SetBinding { slot, binding } => self
                .settings
                .set_binding(slot.into(), binding.trigger(), binding.text())
                .is_ok(),
            Effect::ClearBinding(slot) => self.settings.clear_binding(slot.into()).is_ok(),
            Effect::Bluetooth(action) => {
                change.bluetooth = Some(action);
                false
            }
            Effect::None => false,
        };
        Ok(())
    }

    pub fn model(&self, links: Links) -> Model {
        let mut model = Model::home(links);
        let buffer = self.engine.buffered_cells();
        let count = buffer
            .iter()
            .position(|cell| *cell == 0)
            .unwrap_or(buffer.len());
        let selected = self.selected.to_le_bytes();
        let held = self.held.to_le_bytes();
        let chord = self.chord.to_le_bytes();
        model.bytes = Bounded::from_slice(
            &[
                self.screen,
                selected[0],
                selected[1],
                held[0],
                held[1],
                chord[0],
                chord[1],
                links.online
                    | (u8::from(self.settings.mode == BrailleMode::Ueb1) << 3)
                    | (u8::from(self.host_switch) << 5),
                links.battery[0],
                links.battery[1],
                count as u8,
                self.menu.notice as u8,
                self.menu.slot,
                self.menu.captured,
                self.menu.capture_count,
            ],
            0,
        )
        .unwrap();
        for cell in &buffer[..count] {
            model.bytes.extend_from_slice(&cell.to_le_bytes()).unwrap();
        }
        let binding = match self.screen {
            8 => self.settings.bindings.get(self.selected as usize),
            10 => self.settings.bindings.get(self.menu.slot as usize),
            11 | 12 => Some(&self.menu.draft),
            _ => None,
        };
        let trigger = binding.map_or(&[][..], |b| b.trigger());
        model.bytes.push(trigger.len() as u8).unwrap();
        for cell in trigger {
            model.bytes.extend_from_slice(&cell.to_le_bytes()).unwrap();
        }
        if self.screen == 1 {
            if self.chord != 0 {
                model.detail = self.preview;
            } else if count != 0 {
                let output = self.engine.preview_timeout(&self.settings);
                if !output.is_empty() {
                    model.detail = release_label(output.as_slice(), false, false);
                }
            }
        } else if let Some(binding) = binding {
            model.detail = escaped(binding.text());
        } else if self.menu.notice != Notice::None && self.screen == 7 {
            let _ = model.detail.write_str(self.menu.notice.text());
        }
        model
    }
}
