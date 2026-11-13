pub const KEYBOARD_REPORT_DESCRIPTOR: [u8; 65] = [
    0x05, 0x01, 0x09, 0x06, 0xa1, 0x01, 0x05, 0x07, 0x19, 0xe0, 0x29, 0xe7, 0x15, 0x00, 0x25, 0x01,
    0x75, 0x01, 0x95, 0x08, 0x81, 0x02, 0x95, 0x01, 0x75, 0x08, 0x81, 0x01, 0x95, 0x05, 0x75, 0x01,
    0x05, 0x08, 0x19, 0x01, 0x29, 0x05, 0x91, 0x02, 0x95, 0x01, 0x75, 0x03, 0x91, 0x01, 0x95, 0x06,
    0x75, 0x08, 0x15, 0x00, 0x26, 0xff, 0x00, 0x05, 0x07, 0x19, 0x00, 0x2a, 0xff, 0x00, 0x81, 0x00,
    0xc0,
];

pub const LEFT_CONTROL: u8 = 0x01;
pub const LEFT_SHIFT: u8 = 0x02;
pub const LEFT_ALT: u8 = 0x04;
pub const LEFT_GUI: u8 = 0x08;
pub const RELEASE_REPORT: [u8; 8] = [0; 8];

pub const CONSUMER_REPORT_DESCRIPTOR: [u8; 23] = [
    0x05, 0x0c, 0x09, 0x01, 0xa1, 0x01, 0x15, 0x00, 0x26, 0xff, 0x3f, 0x19, 0x00, 0x2a, 0xff, 0x3f,
    0x75, 0x10, 0x95, 0x01, 0x81, 0x00, 0xc0,
];

pub const BRAILLE_BLE_REPORT_DESCRIPTOR: [u8; 92] = {
    let mut bytes = [0; 92];
    let mut index = 0;
    while index < 6 {
        bytes[index] = KEYBOARD_REPORT_DESCRIPTOR[index];
        index += 1;
    }
    bytes[6] = 0x85;
    bytes[7] = 1;
    while index < 65 {
        bytes[index + 2] = KEYBOARD_REPORT_DESCRIPTOR[index];
        index += 1;
    }
    index = 0;
    while index < 6 {
        bytes[67 + index] = CONSUMER_REPORT_DESCRIPTOR[index];
        index += 1;
    }
    bytes[73] = 0x85;
    bytes[74] = 2;
    while index < 23 {
        bytes[69 + index] = CONSUMER_REPORT_DESCRIPTOR[index];
        index += 1;
    }
    bytes
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum StrokeKind {
    #[default]
    Tap,
    Sticky,
    Held,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyStroke {
    pub usage: u8,
    pub modifiers: u8,
    pub consumer: u16,
    kind: StrokeKind,
}

impl KeyStroke {
    pub const fn new(usage: u8, modifiers: u8) -> Self {
        Self {
            usage,
            modifiers,
            consumer: 0,
            kind: StrokeKind::Tap,
        }
    }

    pub const fn with_consumer(mut self, consumer: u16) -> Self {
        self.consumer = consumer;
        self
    }

    pub const fn sticky(mut self) -> Self {
        self.kind = StrokeKind::Sticky;
        self
    }

    pub const fn held(mut self) -> Self {
        self.kind = StrokeKind::Held;
        self
    }

    pub const fn is_sticky(self) -> bool {
        matches!(self.kind, StrokeKind::Sticky)
    }

    pub const fn is_held(self) -> bool {
        matches!(self.kind, StrokeKind::Held)
    }

    pub const fn consumer_report(self) -> [u8; 2] {
        self.consumer.to_le_bytes()
    }

    pub const fn report(self) -> [u8; 8] {
        [self.modifiers, 0, self.usage, 0, 0, 0, 0, 0]
    }

    pub const fn packed(self) -> u32 {
        self.usage as u32
            | ((self.modifiers as u32) << 8)
            | ((self.consumer as u32) << 16)
            | match self.kind {
                StrokeKind::Tap => 0,
                StrokeKind::Sticky => 1 << 31,
                StrokeKind::Held => 1 << 30,
            }
    }

    pub const fn from_packed(packed: u32) -> Option<Self> {
        let usage = packed as u8;
        let modifiers = (packed >> 8) as u8;
        let consumer = ((packed >> 16) & 0x3fff) as u16;
        let sticky = packed & (1 << 31) != 0;
        if packed == 0
            || packed & (1 << 30) != 0
            || (usage > 0 && usage < 4)
            || (sticky && (usage != 0 || (modifiers == 0 && consumer == 0)))
            || (!sticky && usage == 0 && modifiers == 0 && consumer == 0)
        {
            return None;
        }
        let stroke = Self::new(usage, modifiers).with_consumer(consumer);
        Some(if sticky { stroke.sticky() } else { stroke })
    }

    pub const fn from_ascii(ascii: u8) -> Option<Self> {
        let (usage, shifted) = match ascii {
            b'a'..=b'z' => (ascii - b'a' + 4, false),
            b'A'..=b'Z' => (ascii - b'A' + 4, true),
            b'1'..=b'9' => (ascii - b'1' + 30, false),
            b'0' => (39, false),
            b'\n' | b'\r' => (40, false),
            0x1b => (41, false),
            0x08 => (42, false),
            b'\t' => (43, false),
            b' ' => (44, false),
            b'-' => (45, false),
            b'_' => (45, true),
            b'=' => (46, false),
            b'+' => (46, true),
            b'[' => (47, false),
            b'{' => (47, true),
            b']' => (48, false),
            b'}' => (48, true),
            b'\\' => (49, false),
            b'|' => (49, true),
            b';' => (51, false),
            b':' => (51, true),
            b'\'' => (52, false),
            b'"' => (52, true),
            b'`' => (53, false),
            b'~' => (53, true),
            b',' => (54, false),
            b'<' => (54, true),
            b'.' => (55, false),
            b'>' => (55, true),
            b'/' => (56, false),
            b'?' => (56, true),
            b'!' => (30, true),
            b'@' => (31, true),
            b'#' => (32, true),
            b'$' => (33, true),
            b'%' => (34, true),
            b'^' => (35, true),
            b'&' => (36, true),
            b'*' => (37, true),
            b'(' => (38, true),
            b')' => (39, true),
            _ => return None,
        };
        Some(Self::new(usage, if shifted { LEFT_SHIFT } else { 0 }))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueueFull;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReportQueue<const N: usize> {
    reports: [[u8; 8]; N],
    head: usize,
    len: usize,
}

impl<const N: usize> Default for ReportQueue<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> ReportQueue<N> {
    pub const fn new() -> Self {
        Self {
            reports: [RELEASE_REPORT; N],
            head: 0,
            len: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub const fn remaining_capacity(&self) -> usize {
        N - self.len
    }

    pub fn front(&self) -> Option<&[u8; 8]> {
        if self.is_empty() {
            None
        } else {
            Some(&self.reports[self.head])
        }
    }

    pub fn pop(&mut self) -> Option<[u8; 8]> {
        let report = *self.front()?;
        self.head = (self.head + 1) % N;
        self.len -= 1;
        Some(report)
    }

    pub fn enqueue(&mut self, report: [u8; 8]) -> Result<(), QueueFull> {
        if self.remaining_capacity() == 0 {
            return Err(QueueFull);
        }
        self.reports[(self.head + self.len) % N] = report;
        self.len += 1;
        Ok(())
    }

    pub fn enqueue_tap(&mut self, stroke: KeyStroke) -> Result<(), QueueFull> {
        if self.remaining_capacity() < 2 {
            return Err(QueueFull);
        }
        self.enqueue(stroke.report())?;
        self.enqueue(RELEASE_REPORT)
    }

    pub fn reset(&mut self) {
        self.head = 0;
        self.len = 0;
    }
}
