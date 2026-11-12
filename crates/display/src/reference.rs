use core::fmt::{self, Write};
use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Point, Size},
    mono_font::{
        MonoTextStyle,
        ascii::{FONT_6X10, FONT_7X13_BOLD},
    },
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, PrimitiveStyle},
    text::Text,
};
use kb_core::{
    KeyStroke,
    reference::{Entry, Mode, Page},
};

pub const BACKGROUND: u16 = 0x0841;
pub const SELECTED: u16 = 0x1147;
pub const ACCENT: u16 = 0x5ffb;
pub const MUTED: u16 = 0x94b2;
pub const WHITE: u16 = 0xffff;
pub const SPACE_COLOR: u16 = 0xff48;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    bytes: [u8; 48],
    len: usize,
}

impl Label {
    pub const fn new() -> Self {
        Self {
            bytes: [0; 48],
            len: 0,
        }
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).unwrap()
    }
}

impl Default for Label {
    fn default() -> Self {
        Self::new()
    }
}

impl Write for Label {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if self.len + text.len() > self.bytes.len() {
            return Err(fmt::Error);
        }
        self.bytes[self.len..self.len + text.len()].copy_from_slice(text.as_bytes());
        self.len += text.len();
        Ok(())
    }
}

pub fn label(entry: Entry) -> Label {
    let mut output = Label::new();
    let stroke = entry.stroke();
    if entry.packed > 0xffff {
        let _ = write!(output, "Raw {:08X}", entry.packed);
        return output;
    }
    let shifted = stroke.modifiers & 0x22 != 0;
    for (mask, text) in [(0x11, "Ctrl+"), (0x44, "Alt+"), (0x88, "Gui+")] {
        if stroke.modifiers & mask != 0 {
            let _ = output.write_str(text);
        }
    }
    let normalized = KeyStroke::new(stroke.usage, if shifted { 2 } else { 0 });
    if let Some(ascii) = (33..=126).find(|ascii| KeyStroke::from_ascii(*ascii) == Some(normalized))
    {
        let _ = write!(output, "'{}'", char::from(ascii));
        return output;
    }
    if shifted {
        let _ = output.write_str("Shift+");
    }
    let name = match stroke.usage {
        0 => "Modifiers",
        40 => "Return",
        41 => "Escape",
        42 => "Backspace",
        43 => "Tab",
        44 => "Space",
        57 => "CapsLock",
        70 => "PrintScreen",
        71 => "ScrollLock",
        72 => "Pause",
        73 => "Insert",
        74 => "Home",
        75 => "PageUp",
        76 => "Delete",
        77 => "End",
        78 => "PageDown",
        79 => "Right",
        80 => "Left",
        81 => "Down",
        82 => "Up",
        83 => "NumLock",
        84 => "Keypad /",
        85 => "Keypad *",
        86 => "Keypad -",
        87 => "Keypad +",
        88 => "Keypad Enter",
        98 => "Keypad 0",
        99 => "Keypad .",
        101 => "Menu",
        102 => "Power",
        103 => "Keypad =",
        224 => "Left Ctrl",
        225 => "Left Shift",
        226 => "Left Alt",
        227 => "Left Gui",
        228 => "Right Ctrl",
        229 => "Right Shift",
        230 => "Right Alt",
        231 => "Right Gui",
        _ => "",
    };
    if !name.is_empty() {
        let _ = output.write_str(name);
    } else if (58..=69).contains(&stroke.usage) {
        let _ = write!(output, "F{}", stroke.usage - 57);
    } else if (104..=115).contains(&stroke.usage) {
        let _ = write!(output, "F{}", stroke.usage - 91);
    } else if (89..=97).contains(&stroke.usage) {
        let _ = write!(output, "Keypad {}", stroke.usage - 88);
    } else {
        let _ = write!(output, "HID {:02X}", stroke.usage);
    }
    output
}

pub fn dot_label(cell: u16) -> Label {
    let mut output = Label::new();
    for dot in 0..8 {
        if cell & (1 << dot) != 0 {
            let _ = write!(output, "{}", dot + 1);
        }
    }
    if cell & 0x100 != 0 {
        if cell & 0xff != 0 {
            let _ = output.write_str("+");
        }
        let _ = output.write_str("Space");
    }
    output
}

struct Row<'a> {
    y: usize,
    bytes: &'a mut [u8; 480],
}

impl OriginDimensions for Row<'_> {
    fn size(&self) -> Size {
        Size::new(240, 280)
    }
}

impl DrawTarget for Row<'_> {
    type Color = Rgb565;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            if point.y == self.y as i32 && (0..240).contains(&point.x) {
                let x = point.x as usize;
                self.bytes[2 * x..2 * x + 2].copy_from_slice(&color.into_storage().to_be_bytes());
            }
        }
        Ok(())
    }
}

fn text(row: &mut Row<'_>, text: &str, x: i32, baseline: i32, color: u16, bold: bool) {
    let height = if bold { 13 } else { 10 };
    if (baseline - height..=baseline + 2).contains(&(row.y as i32)) {
        let font = if bold { &FONT_7X13_BOLD } else { &FONT_6X10 };
        let style = MonoTextStyle::new(font, Rgb565::from(RawU16::new(color)));
        let _ = Text::new(text, Point::new(x, baseline), style).draw(row);
    }
}

use embedded_graphics::pixelcolor::raw::RawU16;

fn cell(row: &mut Row<'_>, entry: Entry, x: i32, y: i32) {
    for (dot, column, line) in [
        (1, 0, 0),
        (2, 0, 1),
        (3, 0, 2),
        (4, 1, 0),
        (5, 1, 1),
        (6, 1, 2),
        (7, 0, 3),
        (8, 1, 3),
    ] {
        if line >= entry.dot_rows() as i32 {
            continue;
        }
        let top = y + line * 7;
        if !(top..top + 5).contains(&(row.y as i32)) {
            continue;
        }
        let active = entry.cell & (1 << (dot - 1)) != 0;
        let color = Rgb565::from(RawU16::new(if active { ACCENT } else { MUTED }));
        let style = if active {
            PrimitiveStyle::with_fill(color)
        } else {
            PrimitiveStyle::with_stroke(color, 1)
        };
        let _ = Circle::new(Point::new(x + column * 9, top), 5)
            .into_styled(style)
            .draw(row);
    }
}

pub fn render_row(page: &Page, y: usize, bytes: &mut [u8; 480]) {
    let selected_top = 46 + usize::from(page.selected) * 37;
    let background = if page.mode != Mode::Namespace
        && page.len != 0
        && (selected_top..selected_top + 36).contains(&y)
    {
        SELECTED
    } else {
        BACKGROUND
    };
    for pixel in bytes.chunks_exact_mut(2) {
        pixel.copy_from_slice(&background.to_be_bytes());
    }
    let mut row = Row { y, bytes };
    text(&mut row, "BRAILLE REFERENCE", 10, 18, ACCENT, true);
    if page.mode == Mode::Namespace {
        text(&mut row, "QuickBrailleFox", 10, 57, WHITE, true);
        text(&mut row, "Space + q received", 10, 82, MUTED, false);
        text(&mut row, "Then Space + r", 10, 115, ACCENT, true);
        text(&mut row, "Open mapping reference", 10, 135, WHITE, false);
        text(
            &mut row,
            "Keep Space held, or release",
            10,
            171,
            MUTED,
            false,
        );
        text(
            &mut row,
            "and press it for each chord.",
            10,
            187,
            MUTED,
            false,
        );
        text(&mut row, "Space + c is reserved.", 10, 219, MUTED, false);
        text(
            &mut row,
            "Typing is captured here.",
            10,
            257,
            SPACE_COLOR,
            false,
        );
        return;
    }
    let mut subtitle = Label::new();
    let _ = write!(
        subtitle,
        "Active map  {}-{} of {}",
        if page.len == 0 { 0 } else { page.start + 1 },
        page.start + u16::from(page.len),
        page.total
    );
    text(&mut row, subtitle.as_str(), 10, 35, MUTED, false);
    for (index, entry) in page.entries[..usize::from(page.len)]
        .iter()
        .copied()
        .enumerate()
    {
        let top = 50 + index as i32 * 37;
        cell(&mut row, entry, 12, top);
        let title = label(entry);
        text(&mut row, title.as_str(), 38, top + 10, WHITE, false);
        let dots = dot_label(entry.cell);
        text(
            &mut row,
            dots.as_str(),
            38,
            top + 24,
            if entry.has_space() {
                SPACE_COLOR
            } else {
                MUTED
            },
            false,
        );
        let mut code = Label::new();
        let _ = write!(code, "{:03X}", entry.cell);
        text(&mut row, code.as_str(), 208, top + 24, MUTED, false);
    }
    if page.len == 0 {
        text(
            &mut row,
            "No mappings in active module",
            10,
            90,
            WHITE,
            false,
        );
    }
    let mut prompt = Label::new();
    if page.mode == Mode::Find {
        let _ = prompt.write_str("FIND: press any chord");
    } else if let Some(cell) = page.lookup {
        let _ = write!(
            prompt,
            "{}: {}",
            dot_label(cell).as_str(),
            if page.lookup_found {
                "selected"
            } else {
                "unmapped"
            }
        );
    } else {
        let _ = prompt.write_str("Plain chord: jump to mapping");
    }
    text(&mut row, prompt.as_str(), 10, 244, SPACE_COLOR, false);
    text(
        &mut row,
        "Space+1/4:prev/next  3/6:page",
        10,
        260,
        MUTED,
        false,
    );
    text(
        &mut row,
        "Space+f:find  Space+12:exit",
        10,
        274,
        MUTED,
        false,
    );
}
