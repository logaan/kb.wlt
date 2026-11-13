#![no_std]

#[path = "../../../toucan/display/keyboard.rs"]
pub mod keyboard;
pub mod menu;
#[path = "../../../toucan/display/nice_view.rs"]
pub mod nice_view;
pub mod reference;
pub mod st7789;

use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Size},
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{Circle, PrimitiveStyle},
    text::Text,
};

pub const WIDTH: usize = 128;
pub const HEIGHT: usize = 32;
pub const FRAME_BYTES: usize = WIDTH * HEIGHT / 8;

#[derive(Clone, PartialEq, Eq)]
pub struct Frame {
    bytes: [u8; FRAME_BYTES],
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            bytes: [0; FRAME_BYTES],
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
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            if point.x < 0 || point.y < 0 || point.x >= WIDTH as i32 || point.y >= HEIGHT as i32 {
                continue;
            }
            let byte = &mut self.bytes[(point.y as usize / 8) * WIDTH + point.x as usize];
            let bit = 1 << (point.y as usize % 8);
            match color {
                BinaryColor::On => *byte |= bit,
                BinaryColor::Off => *byte &= !bit,
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Transport {
    #[default]
    Usb,
    Bluetooth,
    Split,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyboardStatus {
    pub active_keys: u8,
    pub layer: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Status {
    pub transport: Transport,
    pub connected: bool,
    pub faulted: bool,
    pub pairing_unsaved: bool,
    pub held: u16,
    pub chord: u16,
    pub last_usage: u8,
    pub last_modifiers: u8,
    pub keyboard: Option<KeyboardStatus>,
}

pub fn render(status: Status) -> Frame {
    let mut frame = Frame::default();
    let style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
    if status.faulted {
        for (label, y) in [
            ("WASM FAULT", 8),
            ("Typing stopped", 19),
            ("Reflash firmware", 30),
        ] {
            let _ = Text::new(label, Point::new(0, y), style).draw(&mut frame);
        }
        return frame;
    }
    let title = if status.pairing_unsaved {
        "PAIRING UNSAVED"
    } else {
        match (status.transport, status.connected) {
            (Transport::Usb, true) => "kb.wlt USB",
            (Transport::Usb, false) => "USB waiting",
            (Transport::Bluetooth, true) => "kb.wlt BLE",
            (Transport::Bluetooth, false) => "BLE pairing",
            (Transport::Split, true) => "kb.wlt SPLIT",
            (Transport::Split, false) => "SPLIT waiting",
        }
    };
    let _ = Text::new(title, Point::new(0, 8), style).draw(&mut frame);
    let mut labels = *b"....... _";
    for (index, label) in labels[..7].iter_mut().enumerate() {
        if status.held & (1 << index) != 0 {
            *label = b'1' + index as u8;
        }
    }
    if status.held & 0x100 != 0 {
        labels[8] = b'S';
    }
    if let Some(keyboard) = status.keyboard {
        labels = *b"KEY 00 L0";
        labels[4] = b'0' + keyboard.active_keys.min(99) / 10;
        labels[5] = b'0' + keyboard.active_keys.min(99) % 10;
        labels[8] = b'0' + keyboard.layer.min(9);
    }
    let _ = Text::new(
        core::str::from_utf8(&labels).unwrap(),
        Point::new(0, 19),
        style,
    )
    .draw(&mut frame);
    let hex = b"0123456789ABCDEF";
    let report = [
        b'H',
        b'I',
        b'D',
        b' ',
        hex[status.last_modifiers as usize >> 4],
        hex[status.last_modifiers as usize & 15],
        b':',
        hex[status.last_usage as usize >> 4],
        hex[status.last_usage as usize & 15],
    ];
    let _ = Text::new(
        core::str::from_utf8(&report).unwrap(),
        Point::new(0, 30),
        style,
    )
    .draw(&mut frame);
    if status.keyboard.is_some() {
        return frame;
    }
    for (dot, x, y) in [
        (1, 108, 0),
        (2, 108, 8),
        (3, 108, 16),
        (7, 108, 24),
        (4, 120, 0),
        (5, 120, 8),
        (6, 120, 16),
        (8, 120, 24),
    ] {
        let active = status.chord & (1 << (dot - 1)) != 0;
        let primitive = if active {
            PrimitiveStyle::with_fill(BinaryColor::On)
        } else {
            PrimitiveStyle::with_stroke(BinaryColor::On, 1)
        };
        let _ = Circle::new(Point::new(x, y), 6)
            .into_styled(primitive)
            .draw(&mut frame);
    }
    frame
}
