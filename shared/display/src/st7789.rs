use crate::{Frame, Status, render};

pub const PANEL_WIDTH: usize = 240;
pub const PANEL_HEIGHT: usize = 280;

#[allow(async_fn_in_trait)]
pub trait PanelBus {
    type Error;
    async fn reset(&mut self);
    async fn delay_ms(&mut self, milliseconds: u32);
    async fn command(&mut self, command: u8, data: &[u8]) -> Result<(), Self::Error>;
    async fn data(&mut self, data: &[u8]) -> Result<(), Self::Error>;
}

pub struct St7789<B> {
    bus: B,
    landscape: bool,
}

impl<B: PanelBus> St7789<B> {
    pub fn new(bus: B) -> Self {
        Self {
            bus,
            landscape: false,
        }
    }

    pub async fn initialize(&mut self) -> Result<(), B::Error> {
        self.bus.reset().await;
        self.landscape = false;
        self.bus.command(0x01, &[]).await?;
        self.bus.delay_ms(150).await;
        for (command, data) in [
            (0x28, &[][..]),
            (0xdf, &[0x5a, 0x69, 0x02, 0x01]),
            (0x36, &[0x00]),
            (0x3a, &[0x05]),
            (0xb2, &[0x0c, 0x0c, 0x00, 0x33, 0x33]),
            (0xb7, &[0x35]),
            (0xbb, &[0x19]),
            (0xc0, &[0x2c]),
            (0xc2, &[0x01]),
            (0xc3, &[0x12]),
            (0xc4, &[0x20]),
            (0xba, &[0x00]),
            (0xc6, &[0x0f]),
            (0x26, &[0x01]),
            (0xd0, &[0xa4, 0xa1]),
            (
                0xe0,
                &[
                    0xd0, 0x04, 0x0d, 0x11, 0x13, 0x2b, 0x3f, 0x54, 0x4c, 0x18, 0x0d, 0x0b, 0x1f,
                    0x23,
                ],
            ),
            (
                0xe1,
                &[
                    0xd0, 0x04, 0x0c, 0x11, 0x13, 0x2c, 0x3f, 0x44, 0x51, 0x2f, 0x1f, 0x1f, 0x20,
                    0x23,
                ],
            ),
            (0xb0, &[0x00, 0xf0]),
            (0xb1, &[0xcd, 0x08, 0x14]),
            (0x21, &[]),
        ] {
            self.bus.command(command, data).await?;
        }
        self.bus.command(0x11, &[]).await?;
        self.bus.delay_ms(120).await;
        self.bus.command(0x13, &[]).await?;
        self.bus.command(0x29, &[]).await?;
        self.bus.delay_ms(20).await;
        Ok(())
    }

    pub async fn present(&mut self, status: Status) -> Result<(), B::Error> {
        if status.keyboard.is_some() {
            self.orientation(true).await?;
            let frame = crate::keyboard::render(status);
            self.bus.command(0x2a, &[0, 20, 1, 43]).await?;
            self.bus.command(0x2b, &[0, 0, 0, 239]).await?;
            self.bus.command(0x2c, &[]).await?;
            let mut row = [0; crate::keyboard::WIDTH * 2];
            for y in 0..crate::keyboard::HEIGHT {
                crate::keyboard::render_row(&frame, status, y, &mut row);
                self.bus.data(&row).await?;
            }
            return Ok(());
        }
        let frame = render(status);
        self.present_rows(|y, row| render_row(&frame, status, y, row))
            .await
    }

    pub async fn present_reference(
        &mut self,
        page: &kb_core::reference::Page,
    ) -> Result<(), B::Error> {
        self.present_rows(|y, row| crate::reference::render_row(page, y, row))
            .await
    }

    pub async fn present_menu(&mut self, page: &crate::menu::Page) -> Result<(), B::Error> {
        self.present_rows(|y, row| crate::menu::render_row(page, y, row))
            .await
    }

    async fn orientation(&mut self, landscape: bool) -> Result<(), B::Error> {
        if self.landscape != landscape {
            self.bus
                .command(0x36, &[if landscape { 0x60 } else { 0x00 }])
                .await?;
            self.landscape = landscape;
        }
        Ok(())
    }

    async fn present_rows(
        &mut self,
        render: impl Fn(usize, &mut [u8; PANEL_WIDTH * 2]),
    ) -> Result<(), B::Error> {
        self.orientation(false).await?;
        self.bus.command(0x2a, &[0, 0, 0, 239]).await?;
        self.bus.command(0x2b, &[0, 20, 1, 43]).await?;
        self.bus.command(0x2c, &[]).await?;
        let mut row = [0; PANEL_WIDTH * 2];
        for y in 0..PANEL_HEIGHT {
            render(y, &mut row);
            self.bus.data(&row).await?;
        }
        Ok(())
    }

    pub fn into_inner(self) -> B {
        self.bus
    }
}

pub fn render_row(frame: &Frame, status: Status, y: usize, row: &mut [u8; PANEL_WIDTH * 2]) {
    for x in 0..PANEL_WIDTH {
        let mut color: u16 = if status.faulted {
            0x7800
        } else if status.pairing_unsaved {
            0x6200
        } else if status.connected {
            0x0184
        } else {
            0x1082
        };
        if (28..=251).contains(&y) && (8..=231).contains(&x) {
            color = 0x0000;
        }
        if (36..100).contains(&y) && (8..232).contains(&x) {
            let source_x = (x - 8) / 2;
            let source_y = (y - 36) / 2;
            if frame.bytes()[source_y / 8 * 128 + source_x] & (1 << (source_y % 8)) != 0 {
                color = 0xffff;
            }
        }
        if let Some(keyboard) = status.keyboard.filter(|_| !status.faulted) {
            if (132..148).contains(&y) && (16..224).contains(&x) {
                color = if x < 16 + usize::from(keyboard.active_keys.min(42)) * 208 / 42 {
                    0x07e0
                } else {
                    0x2104
                };
            }
        }
        for (dot, center_x, center_y) in [
            (1, 92, 134),
            (2, 92, 166),
            (3, 92, 198),
            (7, 92, 230),
            (4, 148, 134),
            (5, 148, 166),
            (6, 148, 198),
            (8, 148, 230),
        ] {
            let dx = x as i32 - center_x;
            let dy = y as i32 - center_y;
            let distance = dx * dx + dy * dy;
            if !status.faulted && status.keyboard.is_none() && distance <= 100 {
                if status.chord & (1 << (dot - 1)) != 0 {
                    color = 0x07e0;
                } else if distance >= 64 {
                    color = 0x632c;
                }
            }
        }
        if !status.faulted
            && status.keyboard.is_none()
            && (92..=148).contains(&x)
            && (243..=247).contains(&y)
        {
            color = if status.held & kb_core::reference::SPACE != 0 {
                0xffe0
            } else {
                0x2104
            };
        }
        row[2 * x..2 * x + 2].copy_from_slice(&color.to_be_bytes());
    }
}
