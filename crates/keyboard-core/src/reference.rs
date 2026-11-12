use crate::KeyStroke;

pub const SPACE: u16 = 0x100;
pub const LEADER: u16 = SPACE | 0x1f;
pub const REFERENCE: u16 = SPACE | 0x17;
pub const NAMESPACE_TIMEOUT_MS: u32 = 2_000;
pub const PAGE_SIZE: usize = 5;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Typing,
    Namespace,
    Reference,
    Find,
    ReleaseGuard,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Action {
    #[default]
    None,
    Namespace,
    Enter,
    Exit,
    Cancel,
    Rejected,
    Previous,
    Next,
    PreviousPage,
    NextPage,
    First,
    Last,
    Find,
    Lookup(u16),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub mode: Mode,
    held: u16,
    dots: u16,
    space: bool,
    space_consumed: bool,
    started_ms: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transition {
    pub state: State,
    pub forward: bool,
    pub reset_braille: bool,
    pub action: Action,
}

impl State {
    pub const fn new() -> Self {
        Self {
            mode: Mode::Typing,
            held: 0,
            dots: 0,
            space: false,
            space_consumed: false,
            started_ms: 0,
        }
    }

    pub const fn captures(self) -> bool {
        !matches!(self.mode, Mode::Typing)
    }

    pub fn cancel(self, keys: u16) -> Transition {
        let mut state = Self::new();
        if keys != 0 {
            state.mode = Mode::ReleaseGuard;
        }
        Transition {
            state,
            forward: false,
            reset_braille: true,
            action: Action::Cancel,
        }
    }

    pub fn step(mut self, keys: u16, now_ms: u32) -> Transition {
        let keys = keys & 0x1ff;
        if self.mode == Mode::ReleaseGuard {
            return self.cancel(keys);
        }
        if self.mode == Mode::Namespace
            && now_ms.wrapping_sub(self.started_ms) >= NAMESPACE_TIMEOUT_MS
        {
            return self.cancel(keys);
        }
        let previous = self.held;
        if previous == 0 && keys != 0 {
            self.space_consumed = false;
        }
        self.held = keys;
        self.dots |= keys & 0xff;
        self.space |= keys & SPACE != 0;
        let cell = if self.dots != 0 && keys & 0xff == 0 {
            let cell = self.dots | if self.space { SPACE } else { 0 };
            self.dots = 0;
            self.space = keys & SPACE != 0;
            self.space_consumed = true;
            Some(cell)
        } else if keys == 0 && previous != 0 {
            let cell = (self.space && !self.space_consumed).then_some(SPACE);
            self.space = false;
            self.space_consumed = false;
            cell
        } else {
            None
        };
        let mut action = Action::None;
        let mut forward = self.mode == Mode::Typing;
        if let Some(cell) = cell {
            match self.mode {
                Mode::Typing if cell == LEADER => {
                    self.mode = Mode::Namespace;
                    self.started_ms = now_ms;
                    action = Action::Namespace;
                    forward = false;
                }
                Mode::Namespace if cell == REFERENCE => {
                    self.mode = Mode::Reference;
                    action = Action::Enter;
                }
                Mode::Namespace => {
                    self.mode = Mode::ReleaseGuard;
                    action = Action::Rejected;
                }
                Mode::Find => {
                    self.mode = Mode::Reference;
                    action = Action::Lookup(cell);
                }
                Mode::Reference => {
                    action = match cell {
                        0x101 => Action::Previous,
                        0x108 => Action::Next,
                        0x104 => Action::PreviousPage,
                        0x120 => Action::NextPage,
                        0x107 => Action::First,
                        0x138 => Action::Last,
                        0x10b => {
                            self.mode = Mode::Find;
                            Action::Find
                        }
                        0x103 => {
                            self.mode = Mode::ReleaseGuard;
                            Action::Exit
                        }
                        _ => Action::Lookup(cell),
                    };
                }
                _ => {}
            }
        }
        Transition {
            state: self,
            forward,
            reset_braille: !forward,
            action,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    pub cell: u16,
    pub packed: u32,
}

impl Entry {
    pub const fn stroke(self) -> KeyStroke {
        KeyStroke::new(self.packed as u8, (self.packed >> 8) as u8)
    }

    pub const fn dot_rows(self) -> usize {
        if self.cell & 0xc0 == 0 { 3 } else { 4 }
    }

    pub const fn has_space(self) -> bool {
        self.cell & SPACE != 0
    }
}

pub struct Catalog {
    entries: [Entry; 511],
    len: u16,
}

impl Catalog {
    pub fn try_from_fn<E>(mut map: impl FnMut(u16) -> Result<u32, E>) -> Result<Self, E> {
        let mut catalog = Self {
            entries: [Entry::default(); 511],
            len: 0,
        };
        for cell in 1..=511 {
            let packed = map(cell)?;
            if packed != 0 {
                catalog.entries[usize::from(catalog.len)] = Entry { cell, packed };
                catalog.len += 1;
            }
        }
        Ok(catalog)
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries[..usize::from(self.len)]
    }

    pub const fn len(&self) -> u16 {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn find(&self, cell: u16) -> Option<u16> {
        self.entries()
            .iter()
            .position(|entry| entry.cell == cell)
            .map(|index| index as u16)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cursor {
    pub selected: u16,
    pub lookup: Option<u16>,
}

impl Cursor {
    pub fn apply(mut self, action: Action, catalog: &Catalog) -> Self {
        let last = catalog.len.saturating_sub(1);
        match action {
            Action::Enter | Action::First => self.selected = 0,
            Action::Last => self.selected = last,
            Action::Previous => self.selected = self.selected.saturating_sub(1),
            Action::Next => self.selected = self.selected.saturating_add(1).min(last),
            Action::PreviousPage => self.selected = self.selected.saturating_sub(PAGE_SIZE as u16),
            Action::NextPage => {
                self.selected = self.selected.saturating_add(PAGE_SIZE as u16).min(last);
            }
            Action::Lookup(cell) => {
                self.lookup = Some(cell);
                if let Some(index) = catalog.find(cell) {
                    self.selected = index;
                }
                return self;
            }
            _ => return self,
        }
        self.lookup = None;
        self
    }

    pub fn page(self, catalog: &Catalog, mode: Mode) -> Page {
        let start = usize::from(self.selected) / PAGE_SIZE * PAGE_SIZE;
        let mut page = Page {
            mode,
            total: catalog.len,
            start: start as u16,
            selected: (usize::from(self.selected) % PAGE_SIZE) as u8,
            lookup: self.lookup,
            lookup_found: self.lookup.and_then(|cell| catalog.find(cell)).is_some(),
            ..Page::default()
        };
        for (destination, entry) in page
            .entries
            .iter_mut()
            .zip(catalog.entries().iter().skip(start))
        {
            *destination = *entry;
            page.len += 1;
        }
        page
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Page {
    pub mode: Mode,
    pub entries: [Entry; PAGE_SIZE],
    pub len: u8,
    pub selected: u8,
    pub total: u16,
    pub start: u16,
    pub lookup: Option<u16>,
    pub lookup_found: bool,
}
