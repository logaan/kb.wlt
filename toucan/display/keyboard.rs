use crate::{
    Status,
    reference::{Label, label},
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
    bytes: [u8; 18 * 120],
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
            let index = point.y as usize * 144 + point.x as usize;
            let mask = 1 << (index % 8);
            match color {
                BinaryColor::On => self.bytes[index / 8] |= mask,
                BinaryColor::Off => self.bytes[index / 8] &= !mask,
            }
        }
        Ok(())
    }
}

fn draw_cell(frame: &mut Frame, cell: u16, x: i32, y: i32, rows: usize) {
    for (i, bit) in [0, 3, 1, 4, 2, 5, 6, 7]
        .into_iter()
        .take(rows * 2)
        .enumerate()
    {
        let dx = x + (i % 2) as i32 * 5;
        let dy = y + (i / 2) as i32 * 4;
        let size = if cell & (1 << bit) != 0 { 3 } else { 1 };
        for row in 0..size {
            for col in 0..size {
                let _ = frame.draw_iter([Pixel(Point::new(dx + col, dy + row), BinaryColor::On)]);
            }
        }
    }
}

pub fn render(status: Status) -> Frame {
    let mut frame = Frame {
        bytes: [0; 18 * 120],
    };
    let small = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
    let layer = status.keyboard.unwrap_or_default().layer;
    let title = if status.pairing_unsaved {
        "PAIRING UNSAVED"
    } else {
        match layer {
            BRAILLE_LAYER => match status.braille.map(|live| live.mode) {
                Some(BrailleMode::Ueb1) => "BRAILLE / UEB 1",
                _ => "BRAILLE / COMPUTER 8",
            },
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
    if layer == BRAILLE_LAYER {
        let live = status.braille.unwrap_or_default();
        let result = live.release.as_str();
        let buffer = if live.buffered[0] == 0 {
            "Buffer: empty"
        } else {
            "Buffer:"
        };
        for (line, y) in [
            (
                if status.connected {
                    "Exit: left outer + P"
                } else {
                    link
                },
                26,
            ),
            ("A S D F    J K L ;", 40),
            ("7 3 2 1    4 5 6 8", 50),
            ("On release:", 65),
            (result.get(..21).unwrap_or(result), 77),
            (result.get(21..).unwrap_or(""), 88),
            (buffer, 103),
            ("Menu: hold Space", 117),
        ] {
            let _ = Text::new(line, Point::new(4, y), small).draw(&mut frame);
        }
        if status.chord != 0 {
            draw_cell(&mut frame, status.chord, 80, 54, 4);
        }
        if status.chord & 256 != 0 {
            let _ = Text::new("Space", Point::new(92, 65), small).draw(&mut frame);
        }
        for (i, cell) in live
            .buffered
            .into_iter()
            .take_while(|cell| *cell != 0)
            .enumerate()
        {
            let x = 49 + i as i32 * 13;
            draw_cell(&mut frame, cell, x, 91, 4);
            if cell & 256 != 0 {
                let _ = Text::new("S", Point::new(x + 7, 102), small).draw(&mut frame);
            }
        }
        for x in [102, 116] {
            draw_cell(&mut frame, kb_core::reference::LEADER, x, 108, 3);
        }
        return frame;
    }
    let _ = Text::new(link, Point::new(4, 26), small).draw(&mut frame);
    let lines = match layer {
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

fn background(status: &Status, y: usize) -> u16 {
    if status.faulted {
        0x7800
    } else if status.pairing_unsaved && y < 58 {
        0x6200
    } else if y < 58 {
        if status.connected { 0x0184 } else { 0x1082 }
    } else if y == 58 {
        0x632c
    } else {
        0
    }
}

pub(crate) fn changed_span(
    old: &Frame,
    previous: &Status,
    frame: &Frame,
    status: &Status,
    y: usize,
) -> Option<core::ops::Range<usize>> {
    if (y..y + 8).any(|row| background(previous, row) != background(status, row)) {
        return Some(0..WIDTH);
    }
    let mut start = 18;
    let mut end = 0;
    for row in y / 2..y / 2 + 4 {
        for col in 0..18 {
            let i = row * 18 + col;
            if old.bytes[i] != frame.bytes[i] {
                start = start.min(col);
                end = end.max(col + 1);
            }
        }
    }
    (end != 0).then_some(start * 16..(end * 16).min(WIDTH))
}

pub(crate) fn render_span(frame: &Frame, status: &Status, y: usize, start: usize, row: &mut [u8]) {
    let bg = background(status, y).to_be_bytes();
    for (i, pixel) in row.chunks_exact_mut(2).enumerate() {
        let x = (start + i) / 2;
        pixel.copy_from_slice(if frame.bytes[(y / 2) * 18 + x / 8] & (1 << (x % 8)) != 0 {
            &[255, 255]
        } else {
            &bg
        });
    }
}

pub fn render_row(frame: &Frame, status: Status, y: usize, row: &mut [u8; WIDTH * 2]) {
    render_span(frame, &status, y, 0, row);
}
