use core::str;

pub const CAPACITY: usize = 768;
const MAX_STROKE_COORD: u16 = 512;
const MAX_STROKE_WIDTH: u8 = 16;
const MAX_ARC_RADIUS: u16 = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scene {
    bytes: [u8; CAPACITY],
    len: u16,
}

impl Scene {
    pub const EMPTY: Self = Self {
        bytes: [0; CAPACITY],
        len: 0,
    };

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > CAPACITY {
            return None;
        }
        let mut remaining = bytes;
        while !remaining.is_empty() {
            let (_, rest) = Command::read(remaining)?;
            remaining = rest;
        }
        let mut scene = Self::EMPTY;
        scene.bytes[..bytes.len()].copy_from_slice(bytes);
        scene.len = bytes.len() as u16;
        Some(scene)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }

    #[cfg(any(feature = "lvgl-tft", feature = "lvgl-mip"))]
    pub(crate) fn commands(&self) -> impl Iterator<Item = Command<'_>> {
        let mut bytes = self.as_bytes();
        core::iter::from_fn(move || {
            let (command, rest) = Command::read(bytes)?;
            bytes = rest;
            Some(command)
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Command<'a> {
    Rect {
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        color: u8,
        radius: u8,
        outline: bool,
    },
    Text {
        x: u16,
        y: u16,
        color: u8,
        size: u8,
        value: &'a str,
    },
    DotGrid {
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        columns: u8,
        rows: u8,
        mask: u8,
        color: u8,
        underline: bool,
        extra_gap: bool,
    },
    Line {
        start: (u16, u16),
        end: (u16, u16),
        color: u8,
        width: u8,
    },
    Arc {
        center: (u16, u16),
        radius: u16,
        start: u16,
        sweep: u8,
        color: u8,
        width: u8,
    },
}

impl<'a> Command<'a> {
    fn read(bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        let [opcode, x0, x1, y0, y1, payload @ ..] = bytes else {
            return None;
        };
        let x = u16::from_le_bytes([*x0, *x1]);
        let y = u16::from_le_bytes([*y0, *y1]);
        if *opcode == 3 {
            let [color, size, len, contents @ ..] = payload else {
                return None;
            };
            let (value, rest) = contents.split_at_checked(usize::from(*len))?;
            let value = str::from_utf8(value).ok()?;
            if *color >= 10 || *size > 3 || value.contains(['\0', '\r', '\n']) {
                return None;
            }
            return Some((
                Self::Text {
                    x,
                    y,
                    color: *color,
                    size: *size,
                    value,
                },
                rest,
            ));
        }
        let [w0, w1, h0, h1, first, second, rest @ ..] = payload else {
            return None;
        };
        let width = u16::from_le_bytes([*w0, *w1]);
        let height = u16::from_le_bytes([*h0, *h1]);
        let command = match opcode {
            1 | 2 if *first < 10 => Self::Rect {
                x,
                y,
                width,
                height,
                color: *first,
                radius: *second,
                outline: *opcode == 2,
            },
            4 => {
                let (grid, rest) = rest.split_first()?;
                let columns = grid & 0xf;
                let rows = grid >> 4;
                let cells = columns * rows;
                let color = second & 0x3f;
                if cells == 0 || cells > 8 || u16::from(*first) >> cells != 0 || color >= 10 {
                    return None;
                }
                return Some((
                    Self::DotGrid {
                        x,
                        y,
                        width,
                        height,
                        columns,
                        rows,
                        mask: *first,
                        color,
                        underline: second & 0x80 != 0,
                        extra_gap: second & 0x40 != 0,
                    },
                    rest,
                ));
            }
            5 | 6
                if x <= MAX_STROKE_COORD
                    && y <= MAX_STROKE_COORD
                    && width <= MAX_STROKE_COORD
                    && height <= MAX_STROKE_COORD
                    && *first < 10
                    && (1..=MAX_STROKE_WIDTH).contains(second) =>
            {
                if *opcode == 5 {
                    Self::Line {
                        start: (x, y),
                        end: (width, height),
                        color: *first,
                        width: *second,
                    }
                } else {
                    let (&sweep, rest) = rest.split_first()?;
                    if !(u16::from(*second)..=MAX_ARC_RADIUS).contains(&width)
                        || height >= 360
                        || sweep == 0
                    {
                        return None;
                    }
                    return Some((
                        Self::Arc {
                            center: (x, y),
                            radius: width,
                            start: height,
                            sweep,
                            color: *first,
                            width: *second,
                        },
                        rest,
                    ));
                }
            }
            _ => return None,
        };
        Some((command, rest))
    }
}
