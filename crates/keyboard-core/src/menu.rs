use crate::{
    KeyStroke,
    preferences::{BINDING_COUNT, Binding, BrailleMode, Error, MAX_TEXT, Settings},
    reference::{Action, Mode},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BluetoothAction {
    Reconnect,
    ClearBonds,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Effect {
    #[default]
    None,
    SetMode(BrailleMode),
    SetBinding {
        slot: u8,
        binding: Binding,
    },
    ClearBinding(u8),
    Bluetooth(BluetoothAction),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Notice {
    #[default]
    None,
    Applied,
    Deleted,
    Busy,
    InvalidText,
    TextFull,
    EmptyText,
    DuplicateTrigger,
    ReservedChord,
    EmptySlot,
    BluetoothRequested,
}

impl Notice {
    pub const fn text(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Applied => "Applied",
            Self::Deleted => "Deleted",
            Self::Busy => "Busy: try again",
            Self::InvalidText => "Not a text character",
            Self::TextFull => "Text limit: 24 characters",
            Self::EmptyText => "Enter expansion text",
            Self::DuplicateTrigger => "Trigger already assigned",
            Self::ReservedChord => "Space+q is reserved",
            Self::EmptySlot => "Choose a trigger length",
            Self::BluetoothRequested => "Bluetooth action requested",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub selected: u8,
    pub slot: u8,
    pub capture_count: u8,
    pub captured: u8,
    pub draft: Binding,
    pub notice: Notice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Update {
    pub state: State,
    pub mode: Mode,
    pub effect: Effect,
}

impl State {
    pub fn enter(self, action: Action) -> Self {
        if matches!(
            action,
            Action::Main | Action::Config | Action::Bindings | Action::Diagnostics
        ) {
            Self::default()
        } else {
            self
        }
    }

    pub fn step(
        mut self,
        mut mode: Mode,
        action: Action,
        settings: &Settings,
        editor_key: Option<KeyStroke>,
    ) -> Update {
        let mut effect = Effect::None;
        self = self.enter(action);
        if mode == Mode::BindingCapture {
            if let Action::Lookup(cell) = action {
                if cell == crate::reference::LEADER {
                    self.notice = Notice::ReservedChord;
                } else if self.captured < self.capture_count && self.captured < 4 {
                    self.draft.trigger[usize::from(self.captured)] = cell;
                    self.captured += 1;
                    self.draft.trigger_len = self.captured;
                    if self.captured == self.capture_count {
                        mode = Mode::BindingText;
                    }
                }
            }
            return Update {
                state: self,
                mode,
                effect,
            };
        }
        if mode == Mode::BindingText {
            match action {
                Action::Back => {
                    mode = Mode::Bindings;
                    self.selected = self.slot;
                    self.notice = Notice::None;
                }
                Action::Activate => {
                    let mut candidate = *settings;
                    match candidate.set_binding(
                        usize::from(self.slot),
                        self.draft.trigger(),
                        self.draft.text(),
                    ) {
                        Ok(()) => {
                            effect = Effect::SetBinding {
                                slot: self.slot,
                                binding: self.draft,
                            };
                            mode = Mode::Bindings;
                            self.selected = self.slot;
                            self.notice = Notice::Applied;
                        }
                        Err(Error::DuplicateTrigger) => self.notice = Notice::DuplicateTrigger,
                        Err(Error::ReservedChord) => self.notice = Notice::ReservedChord,
                        Err(_) => self.notice = Notice::EmptyText,
                    }
                }
                Action::Lookup(_) => {
                    if let Some(key) = editor_key {
                        if key.usage == 42 && key.modifiers == 0 {
                            self.draft.text_len = self.draft.text_len.saturating_sub(1);
                            self.draft.text[usize::from(self.draft.text_len)] = 0;
                            self.notice = Notice::None;
                        } else if let Some(ascii) = ascii(key) {
                            if usize::from(self.draft.text_len) == MAX_TEXT {
                                self.notice = Notice::TextFull;
                            } else {
                                self.draft.text[usize::from(self.draft.text_len)] = ascii;
                                self.draft.text_len += 1;
                                self.notice = Notice::None;
                            }
                        } else {
                            self.notice = Notice::InvalidText;
                        }
                    } else {
                        self.notice = Notice::InvalidText;
                    }
                }
                _ => {}
            }
            return Update {
                state: self,
                mode,
                effect,
            };
        }
        let count = match mode {
            Mode::Main => 4,
            Mode::Config => 3,
            Mode::Bindings => BINDING_COUNT as u8,
            Mode::BindingOptions => 6,
            Mode::Diagnostics => 1,
            _ => 0,
        };
        match action {
            Action::Previous => self.selected = self.selected.saturating_sub(1),
            Action::Next => {
                self.selected = self.selected.saturating_add(1).min(count.saturating_sub(1))
            }
            Action::PreviousPage => self.selected = self.selected.saturating_sub(5),
            Action::NextPage => {
                self.selected = self.selected.saturating_add(5).min(count.saturating_sub(1))
            }
            Action::First => self.selected = 0,
            Action::Last => self.selected = count.saturating_sub(1),
            Action::Back => {
                mode = match mode {
                    Mode::Main => Mode::ReleaseGuard,
                    Mode::BindingOptions => Mode::Bindings,
                    _ => Mode::Main,
                };
                self.selected = if mode == Mode::Bindings { self.slot } else { 0 };
                self.notice = Notice::None;
            }
            Action::Activate => match mode {
                Mode::Main => {
                    mode = match self.selected {
                        0 => Mode::Reference,
                        1 => Mode::Config,
                        2 => Mode::Bindings,
                        _ => Mode::Diagnostics,
                    };
                    self.selected = 0;
                    self.notice = Notice::None;
                }
                Mode::Config => {
                    if self.selected == 0 {
                        effect = Effect::SetMode(match settings.mode {
                            BrailleMode::Computer8 => BrailleMode::Ueb1,
                            BrailleMode::Ueb1 => BrailleMode::Computer8,
                        });
                        self.notice = Notice::Applied;
                    } else {
                        effect = Effect::Bluetooth(match self.selected {
                            1 => BluetoothAction::Reconnect,
                            _ => BluetoothAction::ClearBonds,
                        });
                        self.notice = Notice::BluetoothRequested;
                    }
                }
                Mode::Bindings => {
                    self.slot = self.selected;
                    self.selected = 0;
                    self.notice = Notice::None;
                    mode = Mode::BindingOptions;
                }
                Mode::BindingOptions => {
                    if self.selected < 4 {
                        self.capture_count = self.selected + 1;
                        self.captured = 0;
                        self.draft = Binding::default();
                        self.notice = Notice::None;
                        mode = Mode::BindingCapture;
                    } else if self.selected == 4 {
                        let binding = settings.bindings[usize::from(self.slot)];
                        if binding.is_empty() {
                            self.notice = Notice::EmptySlot;
                        } else {
                            self.draft = binding;
                            self.notice = Notice::None;
                            mode = Mode::BindingText;
                        }
                    } else {
                        effect = Effect::ClearBinding(self.slot);
                        self.notice = Notice::Deleted;
                        self.selected = self.slot;
                        mode = Mode::Bindings;
                    }
                }
                _ => {}
            },
            _ => {}
        }
        Update {
            state: self,
            mode,
            effect,
        }
    }
}

pub fn ascii(stroke: KeyStroke) -> Option<u8> {
    [b'\n', b'\t']
        .into_iter()
        .chain(32..=126)
        .find(|byte| KeyStroke::from_ascii(*byte) == Some(stroke))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Diagnostics {
    pub uptime_seconds: u32,
    pub usb_ready: bool,
    pub peers: u8,
    pub held: u16,
    pub last_usage: u8,
    pub last_modifiers: u8,
    pub component_fault: bool,
    pub pairing_unsaved: bool,
    pub settings_unsaved: bool,
}
