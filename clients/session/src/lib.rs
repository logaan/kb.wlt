#![no_std]

pub mod labels;

use core::fmt::Write;
use kb_core::{
    KeyStroke,
    chord::{CHORD_MASK, RecentChord, SPACE_DOT},
    host_target::{Target, TargetConnection},
    menu::{BluetoothAction, Effect, Notice, State},
    pipeline::Bounded,
    preferences::{BrailleMode, Engine, Fallback, Output, Settings},
    reference::{Action, LEADER, Mode},
};
use labels::{Label, escaped, release_label};

pub const BOOTLOADER_LEFT: u8 = 1;
pub const BOOTLOADER_RIGHT: u8 = 2;
pub const BOOTLOADER_PROSPECTOR: u8 = 4;
const BOOTLOADER_DEVICES: u8 = BOOTLOADER_LEFT | BOOTLOADER_RIGHT | BOOTLOADER_PROSPECTOR;
const BOOTLOADER_SCREEN: u8 = 14;
const HOST_TARGET_SCREEN: u8 = 15;
const HOST_FEEDBACK_SCREEN: u8 = 16;
const STANDALONE_RECEIVER_MENU: u8 = 17;

#[derive(Clone, Copy, Default)]
#[repr(u8)]
enum BootloaderNotice {
    #[default]
    None,
    ChooseDevices,
    DeviceUnavailable,
    Requested,
    Partial,
    Failed,
}

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
        view: Viewport,
    ) -> Result<u32, Self::Error>;
    fn navigate_action(
        &mut self,
        screen: u8,
        selected: u16,
        navigation: Navigation,
        view: Viewport,
    ) -> Result<u32, Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Navigation {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Escape,
}

impl Navigation {
    pub const fn from_byte(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Up),
            1 => Some(Self::Down),
            2 => Some(Self::Left),
            3 => Some(Self::Right),
            4 => Some(Self::Enter),
            5 => Some(Self::Escape),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Viewport {
    pub width: u16,
    pub height: u16,
    pub host_switch: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Links {
    pub online: u8,
    pub battery: [u8; 2],
    pub target: Target,
    pub connection: TargetConnection,
}

impl Default for Links {
    fn default() -> Self {
        Self {
            online: 0,
            battery: [255; 2],
            target: Target::Unknown,
            connection: TargetConnection::Unknown,
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
        let mut model = [0; 17];
        model[7] = links.online;
        model[8..10].copy_from_slice(&links.battery);
        model[15] = links.target as u8;
        model[16] = links.connection as u8;
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
    pub target: Option<Target>,
    pub bootloader: u8,
    pub reset: bool,
}

pub struct Session {
    pub settings: Settings,
    screen: u8,
    selected: u16,
    help_return: (u8, u16),
    viewport: (u16, u16),
    host_switch: bool,
    host_target: Target,
    standalone_receiver_menu: bool,
    menu: State,
    bootloader_devices: u8,
    bootloader_selected: u8,
    bootloader_notice: BootloaderNotice,
    engine: Engine,
    held: u16,
    chord: u16,
    capture: RecentChord,
    space_consumed: bool,
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
            host_target: Target::default(),
            standalone_receiver_menu: false,
            menu: State::default(),
            bootloader_devices: 0,
            bootloader_selected: 0,
            bootloader_notice: BootloaderNotice::None,
            engine: Engine::default(),
            held: 0,
            chord: 0,
            capture: RecentChord::new(),
            space_consumed: false,
            guard: false,
            fallback: Fallback::default(),
            preview: Label::new(),
        }
    }

    pub fn set_viewport(&mut self, width: u16, height: u16) {
        self.viewport = (width, height);
    }

    pub fn enable_host_switch(&mut self) {
        self.host_switch = true;
    }

    pub fn set_host_target(&mut self, target: Target) {
        self.host_target = target;
    }

    pub fn enable_standalone_receiver_menu(&mut self) {
        self.standalone_receiver_menu = true;
    }

    pub fn menu_open(&self) -> bool {
        self.screen >= 2 && self.screen != HOST_FEEDBACK_SCREEN
    }

    pub fn navigate_action<G: Guest>(
        &mut self,
        navigation: Navigation,
        client: &mut G,
    ) -> Result<Change, G::Error> {
        let mut change = Change::default();
        let keys = self.capture.held();
        if !self.menu_open() {
            self.cancel(keys);
            change.reset = true;
        }
        let packed = client.navigate_action(self.screen, self.selected, navigation, self.view())?;
        self.apply_navigation(packed, None, client, &mut change)?;
        if self.screen == 1 {
            self.cancel(keys);
        } else if self.screen == HOST_FEEDBACK_SCREEN {
            self.cancel_for_link_change(keys);
        }
        Ok(change)
    }

    pub fn set_bootloader_devices(&mut self, available: u8) {
        self.bootloader_devices = available & BOOTLOADER_DEVICES;
        if self.bootloader_selected & !self.bootloader_devices != 0 {
            self.bootloader_selected &= self.bootloader_devices;
            if !matches!(
                self.bootloader_notice,
                BootloaderNotice::Requested | BootloaderNotice::Partial | BootloaderNotice::Failed
            ) {
                self.bootloader_notice = BootloaderNotice::DeviceUnavailable;
            }
        }
    }

    pub fn bootloader_result(&mut self, requested: u8, accepted: u8) {
        let accepted = requested & accepted;
        self.bootloader_selected = requested & !accepted & self.bootloader_devices;
        self.bootloader_notice = if accepted == 0 {
            BootloaderNotice::Failed
        } else if accepted == requested {
            BootloaderNotice::Requested
        } else {
            BootloaderNotice::Partial
        };
    }

    pub fn cancel(&mut self, keys: u16) {
        let viewport = self.viewport;
        let host_switch = self.host_switch;
        let host_target = self.host_target;
        let standalone_receiver_menu = self.standalone_receiver_menu;
        let bootloader_devices = self.bootloader_devices;
        *self = Self::new(self.settings);
        self.viewport = viewport;
        self.host_switch = host_switch;
        self.host_target = host_target;
        self.standalone_receiver_menu = standalone_receiver_menu;
        self.bootloader_devices = bootloader_devices;
        self.guard = keys != 0;
    }

    pub fn cancel_for_link_change(&mut self, keys: u16) {
        self.engine.reset();
        self.held = 0;
        self.chord = 0;
        self.capture = RecentChord::new();
        self.space_consumed = false;
        self.fallback = Fallback::default();
        self.preview = Label::new();
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

    fn menu_chord(&mut self, previous: u16, keys: u16, candidate: u16) -> Option<u16> {
        const DOTS: u16 = CHORD_MASK ^ SPACE_DOT;
        if previous & DOTS != 0 && keys & DOTS == 0 && (self.screen != 1 || candidate == LEADER) {
            self.capture.clear_released();
            self.space_consumed = keys != 0;
            return Some(candidate);
        }
        if previous != 0 && keys == 0 {
            let consumed = core::mem::take(&mut self.space_consumed);
            if self.screen != 1 && !consumed && candidate == SPACE_DOT {
                return Some(SPACE_DOT);
            }
        }
        None
    }

    pub fn update<G: Guest>(
        &mut self,
        keys: u16,
        now: u32,
        client: &mut G,
    ) -> Result<Change, G::Error> {
        let keys = keys & CHORD_MASK;
        let mut change = Change::default();
        if self.guard {
            self.guard = keys != 0;
            return Ok(change);
        }
        if self.screen == HOST_FEEDBACK_SCREEN {
            if keys == 0 {
                return Ok(change);
            }
            self.screen = 1;
        }
        let previous = self.capture.held();
        let candidate = self.capture.update(keys, now);
        let cell = self.menu_chord(previous, keys, candidate);
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
                } else if self.screen == HOST_FEEDBACK_SCREEN {
                    self.cancel_for_link_change(keys);
                }
            }
        } else {
            change.strokes = self.type_chord(previous, keys, candidate, now, client)?;
        }
        Ok(change)
    }

    fn type_chord<G: Guest>(
        &mut self,
        previous: u16,
        keys: u16,
        candidate: u16,
        now: u32,
        client: &mut G,
    ) -> Result<Output, G::Error> {
        let known = self.chord & candidate;
        let changed = keys != previous || candidate != self.chord;
        if known != self.chord {
            self.fallback = Fallback::default();
        }
        if keys != previous {
            let (held, _, stroke) = client.step(self.held, known, keys)?;
            self.held = held;
            if let Some(stroke) = stroke {
                let _ = self.fallback.push(stroke);
            }
        }
        if keys == 0 && previous != 0 {
            self.chord = 0;
            self.preview = Label::new();
            return Ok(self.engine.accept_batch(
                &self.settings,
                candidate,
                core::mem::take(&mut self.fallback),
                now,
            ));
        }
        self.chord = candidate;
        let output = self.engine.poll(&self.settings, now);
        if keys != 0 && (changed || !output.is_empty()) {
            let mut engine = self.engine;
            let mut batch = self.fallback;
            if let Some(stroke) = KeyStroke::from_packed(client.map_cell(candidate)?) {
                let _ = batch.push(stroke);
            }
            let result = engine.accept_batch(&self.settings, candidate, batch, now);
            self.preview = release_label(result.as_slice(), candidate == LEADER);
        }
        Ok(output)
    }

    fn navigate<G: Guest>(
        &mut self,
        cell: u16,
        client: &mut G,
        change: &mut Change,
    ) -> Result<(), G::Error> {
        let packed = client.navigate(self.screen, self.selected, cell, self.view())?;
        self.apply_navigation(packed, Some(cell), client, change)
    }

    fn view(&self) -> Viewport {
        let (width, height) = self.viewport;
        Viewport {
            width,
            height,
            host_switch: self.host_switch,
        }
    }

    fn apply_navigation<G: Guest>(
        &mut self,
        packed: u32,
        cell: Option<u16>,
        client: &mut G,
        change: &mut Change,
    ) -> Result<(), G::Error> {
        let previous = (self.screen, self.selected);
        let action = (packed >> 24) as u8;
        self.screen = packed as u8;
        self.selected = (packed >> 8) as u16;
        if self.standalone_receiver_menu && matches!(self.screen, 2 | 6) {
            self.screen = STANDALONE_RECEIVER_MENU;
            self.selected = self.selected.min(1);
        }
        if self.screen == HOST_TARGET_SCREEN && previous.0 != HOST_TARGET_SCREEN && previous.0 != 13
        {
            self.selected = u16::from(self.host_target as u8).min(u16::from(Target::Usb as u8));
        }
        if previous.0 == HOST_TARGET_SCREEN && self.screen == HOST_FEEDBACK_SCREEN {
            change.target = u8::try_from(previous.1).ok().and_then(Target::from_byte);
            if let Some(target) = change.target {
                self.host_target = target;
            }
            return Ok(());
        }
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
        if previous.0 == BOOTLOADER_SCREEN {
            if action == 1 {
                change.bootloader = self.bootloader_activate(previous.1);
            } else if self.screen != 13 && self.screen != BOOTLOADER_SCREEN {
                self.bootloader_selected = 0;
                self.bootloader_notice = BootloaderNotice::None;
            }
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
            3 => match cell {
                Some(cell) => Action::Lookup(cell),
                None => return Ok(()),
            },
            _ => return Ok(()),
        };
        self.menu.selected = previous.1 as u8;
        let stroke = match command {
            Action::Lookup(cell) => KeyStroke::from_packed(client.map_cell(cell)?),
            _ => None,
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

    fn bootloader_activate(&mut self, index: u16) -> u8 {
        if let Some(device) = [BOOTLOADER_LEFT, BOOTLOADER_RIGHT, BOOTLOADER_PROSPECTOR]
            .get(index as usize)
            .copied()
        {
            if self.bootloader_devices & device != 0 {
                self.bootloader_selected ^= device;
                self.bootloader_notice = BootloaderNotice::None;
            } else {
                self.bootloader_notice = BootloaderNotice::DeviceUnavailable;
            }
            return 0;
        }
        if index != 3 {
            return 0;
        }
        let requested = core::mem::take(&mut self.bootloader_selected);
        self.bootloader_notice = if requested == 0 {
            BootloaderNotice::ChooseDevices
        } else {
            BootloaderNotice::Requested
        };
        requested
    }

    pub fn model(&self, links: Links) -> Model {
        let mut model = Model::home(links);
        if self.screen == HOST_FEEDBACK_SCREEN {
            return model;
        }
        let buffer = self.engine.buffered_cells();
        let count = buffer
            .iter()
            .position(|cell| *cell == 0)
            .unwrap_or(buffer.len());
        let selected = self.selected.to_le_bytes();
        let held = self.capture.held().to_le_bytes();
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
        if self.screen == BOOTLOADER_SCREEN {
            model
                .bytes
                .extend_from_slice(&[
                    self.bootloader_devices,
                    self.bootloader_selected,
                    self.bootloader_notice as u8,
                ])
                .unwrap();
            return model;
        }
        if matches!(self.screen, 9 | HOST_TARGET_SCREEN) {
            model
                .bytes
                .extend_from_slice(&[links.target as u8, links.connection as u8])
                .unwrap();
            return model;
        }
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
                    model.detail = release_label(output.as_slice(), false);
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
