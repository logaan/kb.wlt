use crate::{Status, Transport};
use core::convert::Infallible;
use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Size},
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{Circle, PrimitiveStyle},
    text::Text,
};

pub const WIDTH: usize = 160;
pub const HEIGHT: usize = 68;
pub const FRAME_BYTES: usize = WIDTH * HEIGHT / 8;
pub const TRANSFER_BYTES: usize = 1 + HEIGHT * (WIDTH / 8 + 2) + 1;

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
        Size::new(68, 160)
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
            if point.x < 0 || point.x >= 68 || point.y < 0 || point.y >= 160 {
                continue;
            }
            let x = point.y as usize;
            let y = 67 - point.x as usize;
            let byte = &mut self.bytes[y * 20 + x / 8];
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
    if status.faulted {
        for (label, y) in [
            ("WASM FAULT", 12),
            ("Typing", 44),
            ("stopped", 60),
            ("Reflash", 92),
            ("firmware", 108),
        ] {
            let _ = Text::new(label, Point::new(4, y), style).draw(&mut frame);
        }
        return frame;
    }
    let transport = match status.transport {
        Transport::Usb => "USB",
        Transport::Bluetooth => "Bluetooth",
        Transport::Split => "Split",
    };
    for (text, y) in [
        (
            if status.pairing_unsaved {
                "PAIRING"
            } else {
                "kb.wlt"
            },
            12,
        ),
        (
            if status.pairing_unsaved {
                "UNSAVED"
            } else {
                transport
            },
            28,
        ),
        (
            if status.connected {
                "Connected"
            } else {
                "Waiting"
            },
            44,
        ),
    ] {
        let _ = Text::new(text, Point::new(4, y), style).draw(&mut frame);
    }
    let digits = b"0123456789ABCDEF";
    let code = [
        b'H',
        b'I',
        b'D',
        b' ',
        digits[status.last_modifiers as usize >> 4],
        digits[status.last_modifiers as usize & 15],
        b':',
        digits[status.last_usage as usize >> 4],
        digits[status.last_usage as usize & 15],
    ];
    let _ = Text::new(
        core::str::from_utf8(&code).unwrap(),
        Point::new(4, 62),
        style,
    )
    .draw(&mut frame);
    if let Some(keyboard) = status.keyboard {
        let keys = [
            b'K',
            b'e',
            b'y',
            b's',
            b' ',
            b'0' + keyboard.active_keys.min(99) / 10,
            b'0' + keyboard.active_keys.min(99) % 10,
        ];
        let layer = [
            b'L',
            b'a',
            b'y',
            b'e',
            b'r',
            b' ',
            b'0' + keyboard.layer.min(9),
        ];
        let _ = Text::new(
            core::str::from_utf8(&keys).unwrap(),
            Point::new(4, 88),
            style,
        )
        .draw(&mut frame);
        let _ = Text::new(
            core::str::from_utf8(&layer).unwrap(),
            Point::new(4, 108),
            style,
        )
        .draw(&mut frame);
        return frame;
    }
    for (dot, x, y) in [
        (1, 14, 76),
        (2, 14, 96),
        (3, 14, 116),
        (7, 14, 136),
        (4, 44, 76),
        (5, 44, 96),
        (6, 44, 116),
        (8, 44, 136),
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
    frame
}

pub fn encode_frame(frame: &Frame, vcom: bool, output: &mut [u8; TRANSFER_BYTES]) {
    output[0] = 0x01 | if vcom { 0x02 } else { 0 };
    for line in 0..HEIGHT {
        let start = 1 + line * 22;
        output[start] = line as u8 + 1;
        output[start + 1..start + 21].copy_from_slice(&frame.bytes[line * 20..(line + 1) * 20]);
        output[start + 21] = 0;
    }
    output[TRANSFER_BYTES - 1] = 0;
}

pub const fn idle_command(vcom: bool) -> [u8; 2] {
    [if vcom { 2 } else { 0 }, 0]
}
pub const fn clear_command(vcom: bool) -> [u8; 2] {
    [4 | if vcom { 2 } else { 0 }, 0]
}
