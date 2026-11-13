use crate::{Status, reference::Label};
use core::fmt::Write;
use embedded_graphics::{
    mono_font::{
        MonoTextStyle,
        ascii::{FONT_6X10, FONT_10X20},
    },
    pixelcolor::BinaryColor,
    prelude::*,
    text::Text,
};

pub const WIDTH: usize = 280;
pub const HEIGHT: usize = 240;

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
    let large = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
    let title = if status.pairing_unsaved {
        "PAIRING UNSAVED"
    } else {
        "TOUCAN"
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
    let _ = Text::new("HELD", Point::new(4, 42), small).draw(&mut frame);
    let _ = Text::new("LAYER", Point::new(76, 42), small).draw(&mut frame);
    let keyboard = status.keyboard.unwrap_or_default();
    for (value, x) in [(keyboard.active_keys, 4), (keyboard.layer, 76)] {
        let mut label = Label::new();
        let _ = write!(label, "{value}");
        let _ = Text::new(label.as_str(), Point::new(x, 64), large).draw(&mut frame);
    }
    for (label, baseline) in [("LAST USB HID REPORT", 81), ("KEY", 96), ("MODIFIERS", 110)] {
        let _ = Text::new(label, Point::new(4, baseline), small).draw(&mut frame);
    }
    for (value, baseline) in [(status.last_usage, 96), (status.last_modifiers, 110)] {
        let mut label = Label::new();
        let _ = write!(label, "0x{value:02X}");
        let _ = Text::new(label.as_str(), Point::new(106, baseline), small).draw(&mut frame);
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
