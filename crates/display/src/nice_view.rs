use crate::{Status, Transport};
use core::convert::Infallible;
use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Size},
    mono_font::{
        MonoTextStyle,
        ascii::{FONT_6X10, FONT_10X20},
    },
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{Circle, PrimitiveStyle, Rectangle},
    text::Text,
};

pub const WIDTH: usize = 144;
pub const HEIGHT: usize = 168;
pub const ROW_BYTES: usize = WIDTH / 8;
pub const FRAME_BYTES: usize = ROW_BYTES * HEIGHT;
pub const TRANSFER_BYTES: usize = 1 + HEIGHT * (ROW_BYTES + 2) + 1;

pub struct Frame {
    bytes: [u8; FRAME_BYTES],
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            bytes: [0xff; FRAME_BYTES],
        }
    }
}

impl Frame {
    pub fn bytes(&self) -> &[u8; FRAME_BYTES] {
        &self.bytes
    }
}

impl OriginDimensions for Frame {
    fn size(&self) -> Size {
        Size::new(WIDTH as u32, HEIGHT as u32)
    }
}

impl DrawTarget for Frame {
    type Color = BinaryColor;
    type Error = Infallible;
    fn draw_iter<I: IntoIterator<Item = Pixel<BinaryColor>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Infallible> {
        for Pixel(point, color) in pixels {
            if point.x < 0 || point.x >= WIDTH as i32 || point.y < 0 || point.y >= HEIGHT as i32 {
                continue;
            }
            let x = point.x as usize;
            let y = point.y as usize;
            let byte = &mut self.bytes[y * ROW_BYTES + x / 8];
            let bit = 1 << (x % 8);
            match color {
                BinaryColor::On => *byte &= !bit,
                BinaryColor::Off => *byte |= bit,
            }
        }
        Ok(())
    }
}

pub fn render(status: Status) -> Frame {
    let mut frame = Frame::default();
    let style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
    let large = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
    let inverse = MonoTextStyle::new(&FONT_6X10, BinaryColor::Off);
    let _ = Rectangle::new(Point::zero(), Size::new(WIDTH as u32, 24))
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
        .draw(&mut frame);
    let title = if status.faulted {
        "WASM FAULT"
    } else if status.pairing_unsaved {
        "PAIRING UNSAVED"
    } else {
        "kb.wlt / TOUCAN"
    };
    let _ = Text::new(title, Point::new(8, 16), inverse).draw(&mut frame);
    if status.faulted {
        for (label, y) in [
            ("Typing stopped", 51),
            ("Reflash firmware", 75),
            ("to resume.", 91),
        ] {
            let _ = Text::new(label, Point::new(8, y), style).draw(&mut frame);
        }
        return frame;
    }
    let transport = match status.transport {
        Transport::Usb => "USB",
        Transport::Bluetooth => "Bluetooth",
        Transport::Split => "Split receiver",
    };
    let _ = Text::new(transport, Point::new(8, 40), style).draw(&mut frame);
    let indicator = if status.connected {
        PrimitiveStyle::with_fill(BinaryColor::On)
    } else {
        PrimitiveStyle::with_stroke(BinaryColor::On, 1)
    };
    let _ = Circle::new(Point::new(8, 49), 7)
        .into_styled(indicator)
        .draw(&mut frame);
    let _ = Text::new(
        if status.connected {
            "Connected"
        } else {
            "Waiting for link"
        },
        Point::new(22, 56),
        style,
    )
    .draw(&mut frame);
    let _ = Rectangle::new(Point::new(8, 66), Size::new(128, 1))
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
        .draw(&mut frame);
    let digits = b"0123456789ABCDEF";
    if let Some(keyboard) = status.keyboard {
        let keys = [
            b'0' + keyboard.active_keys.min(99) / 10,
            b'0' + keyboard.active_keys.min(99) % 10,
        ];
        let layer = [b'0' + keyboard.layer.min(9)];
        for (label, x) in [("KEYS HELD", 8), ("LAYER", 86)] {
            let _ = Text::new(label, Point::new(x, 82), style).draw(&mut frame);
        }
        for (value, x) in [(keys.as_slice(), 8), (layer.as_slice(), 86)] {
            let _ = Text::new(
                core::str::from_utf8(value).unwrap(),
                Point::new(x, 108),
                large,
            )
            .draw(&mut frame);
        }
        let _ = Rectangle::new(Point::new(8, 119), Size::new(128, 1))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(&mut frame);
        let _ = Text::new("LAST HID / HEX", Point::new(8, 136), style).draw(&mut frame);
        let report = [
            b'M',
            b'O',
            b'D',
            b' ',
            digits[status.last_modifiers as usize >> 4],
            digits[status.last_modifiers as usize & 15],
            b' ',
            b' ',
            b'K',
            b'E',
            b'Y',
            b' ',
            digits[status.last_usage as usize >> 4],
            digits[status.last_usage as usize & 15],
        ];
        let _ = Text::new(
            core::str::from_utf8(&report).unwrap(),
            Point::new(8, 154),
            style,
        )
        .draw(&mut frame);
    } else {
        for (dot, x, y) in [
            (1, 48, 78),
            (2, 48, 98),
            (3, 48, 118),
            (7, 48, 138),
            (4, 86, 78),
            (5, 86, 98),
            (6, 86, 118),
            (8, 86, 138),
        ] {
            let active = if dot == 8 {
                status.held & 0x100 != 0
            } else {
                status.chord & (1 << (dot - 1)) != 0
            };
            let stroke = if active {
                PrimitiveStyle::with_fill(BinaryColor::On)
            } else {
                PrimitiveStyle::with_stroke(BinaryColor::On, 1)
            };
            let _ = Circle::new(Point::new(x, y), 10)
                .into_styled(stroke)
                .draw(&mut frame);
        }
    }
    frame
}

pub fn encode_frame(frame: &Frame, vcom: bool, output: &mut [u8; TRANSFER_BYTES]) {
    output[0] = 0x01 | if vcom { 0x02 } else { 0 };
    for line in 0..HEIGHT {
        let start = 1 + line * (ROW_BYTES + 2);
        output[start] = line as u8 + 1;
        output[start + 1..start + 1 + ROW_BYTES]
            .copy_from_slice(&frame.bytes[line * ROW_BYTES..(line + 1) * ROW_BYTES]);
        output[start + 1 + ROW_BYTES] = 0;
    }
    output[TRANSFER_BYTES - 1] = 0;
}

pub const fn idle_command(vcom: bool) -> [u8; 2] {
    [if vcom { 2 } else { 0 }, 0]
}
pub const fn clear_command(vcom: bool) -> [u8; 2] {
    [4 | if vcom { 2 } else { 0 }, 0]
}
