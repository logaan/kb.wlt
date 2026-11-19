use crate::{
    Status,
    reference::{Label, dot_label, label},
};
use core::fmt::Write;
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::BinaryColor,
    prelude::*,
    text::Text,
};
use kb_core::{
    KeyStroke,
    preferences::{BrailleMode, MAX_TRIGGER},
    reference::Entry,
};

pub const BRAILLE_LAYER: u8 = 2;
pub const WIDTH: usize = 280;
pub const HEIGHT: usize = 240;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LiveBraille {
    pub mode: BrailleMode,
    pub buffered: [u16; MAX_TRIGGER + 3],
    pub release: Label,
}

pub fn release_label(strokes: &[KeyStroke], waiting: bool, leader: bool) -> Label {
    let mut result = Label::new();
    if leader {
        let _ = result.write_str("Menu prefix");
        return result;
    }
    for stroke in strokes.iter().copied() {
        if stroke.is_held() && stroke.consumer == 0 && stroke.modifiers == 0 {
            continue;
        }
        let stroke = if stroke.is_held() {
            KeyStroke::new(0, stroke.modifiers)
                .with_consumer(stroke.consumer)
                .sticky()
        } else {
            stroke
        };
        let name = label(Entry {
            cell: 0,
            packed: stroke.packed(),
        });
        let text = name.as_str();
        let short = if text.len() == 3 && text.starts_with('\'') {
            &text[1..2]
        } else if text == "Space" && strokes.len() > 1 {
            " "
        } else {
            text
        };
        if result.as_str().len() + short.len() > 39 {
            let _ = result.write_str("...");
            break;
        }
        let _ = result.write_str(short);
    }
    if result.as_str().is_empty() {
        let _ = result.write_str(if waiting {
            "Wait for next chord"
        } else {
            "No text"
        });
    }
    result
}

pub struct Frame {
    bytes: [u8; 140 * 120 / 8],
}

impl OriginDimensions for Frame {
    fn size(&self) -> Size {
        Size::new(140, 120)
    }
}

impl DrawTarget for Frame {
    type Color = BinaryColor;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            if !(0..140).contains(&point.x) || !(0..120).contains(&point.y) {
                continue;
            }
            let index = point.y as usize * 140 + point.x as usize;
            let mask = 1 << (index % 8);
            match color {
                BinaryColor::On => self.bytes[index / 8] |= mask,
                BinaryColor::Off => self.bytes[index / 8] &= !mask,
            }
        }
        Ok(())
    }
}

pub fn render(status: Status) -> Frame {
    let mut frame = Frame { bytes: [0; 2100] };
    let small = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
    let layer = status.keyboard.unwrap_or_default().layer;
    let title = if status.pairing_unsaved {
        "PAIRING UNSAVED"
    } else {
        match layer {
            BRAILLE_LAYER => "TOUCAN / BRAILLE",
            1 => "TOUCAN / NAVIGATION",
            _ => "TOUCAN / TYPING",
        }
    };
    let _ = Text::new(title, Point::new(8, 16), small).draw(&mut frame);
    if status.faulted {
        for (label, baseline) in [
            ("WASM FAULT", 38),
            ("Typing stopped", 64),
            ("Reflash firmware", 90),
        ] {
            let _ = Text::new(label, Point::new(4, baseline), small).draw(&mut frame);
        }
        return frame;
    }
    let link = if status.connected {
        "CONNECTED"
    } else {
        "WAITING FOR HALVES"
    };
    let _ = Text::new(link, Point::new(4, 26), small).draw(&mut frame);
    if let Some(live) = status.braille {
        if status.chord != 0 || live.buffered[0] != 0 {
            let mut top = Label::new();
            let _ = write!(
                top,
                "{}: {}",
                live.mode.name(),
                dot_label(status.chord).as_str()
            );
            for (line, y) in [
                (top.as_str(), 40),
                ("On release:", 52),
                (
                    live.release
                        .as_str()
                        .get(..21)
                        .unwrap_or(live.release.as_str()),
                    64,
                ),
                (live.release.as_str().get(21..).unwrap_or(""), 75),
                (
                    if live.buffered[0] == 0 {
                        "Buffer: empty"
                    } else {
                        "Buffer (oldest first)"
                    },
                    89,
                ),
            ] {
                let _ = Text::new(line, Point::new(4, y), small).draw(&mut frame);
            }
            for (i, cell) in live
                .buffered
                .into_iter()
                .take_while(|cell| *cell != 0)
                .enumerate()
            {
                let x = 6 + i as i32 * 18;
                for (bit, dx, dy) in [
                    (0, 0, 0),
                    (1, 0, 4),
                    (2, 0, 8),
                    (6, 0, 12),
                    (3, 5, 0),
                    (4, 5, 4),
                    (5, 5, 8),
                    (7, 5, 12),
                ] {
                    let on = cell & (1 << bit) != 0;
                    for (a, b) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                        if on || a + b == 0 {
                            let _ = frame.draw_iter([Pixel(
                                Point::new(x + dx + a, 94 + dy + b),
                                BinaryColor::On,
                            )]);
                        }
                    }
                }
                if cell & 256 != 0 {
                    let _ = Text::new("S", Point::new(x, 118), small).draw(&mut frame);
                }
            }
            return frame;
        }
    }
    let dots = status.braille.map_or("", |live| live.mode.name());
    let lines = match layer {
        BRAILLE_LAYER => [
            "A S D F    J K L ;",
            "7 3 2 1    4 5 6 8",
            dots,
            "Menu: hold Space",
            "Chord SDFJK twice",
            "Exit: left outer + P",
        ],
        1 => [
            "W Left      O Right",
            "R Up        I Down",
            "",
            "P: enter braille",
            "Release thumb to type",
            "",
        ],
        _ => [
            "Hold left outer thumb",
            "for arrow keys.",
            "",
            "To enter braille:",
            "Hold left outer thumb",
            "then press P.",
        ],
    };
    for (i, line) in lines.iter().enumerate() {
        let _ = Text::new(line, Point::new(5, 42 + 14 * i as i32), small).draw(&mut frame);
    }
    frame
}

pub fn render_row(frame: &Frame, status: Status, y: usize, row: &mut [u8; WIDTH * 2]) {
    for x in 0..WIDTH {
        let index = (y / 2) * 140 + x / 2;
        let color: u16 = if frame.bytes[index / 8] & (1 << (index % 8)) != 0 {
            0xffff
        } else if status.faulted {
            0x7800
        } else if status.pairing_unsaved && y < 58 {
            0x6200
        } else if y < 58 {
            if status.connected { 0x0184 } else { 0x1082 }
        } else if y == 58 || y == 140 {
            0x632c
        } else {
            0x0000
        };
        row[x * 2..x * 2 + 2].copy_from_slice(&color.to_be_bytes());
    }
}
