#[allow(async_fn_in_trait)]
pub trait PanelBus {
    type Error;
    async fn reset(&mut self);
    async fn delay_ms(&mut self, milliseconds: u32);
    async fn command(&mut self, command: u8, data: &[u8]) -> Result<(), Self::Error>;
    async fn data(&mut self, data: &[u8]) -> Result<(), Self::Error>;
}

async fn initialize<B: PanelBus>(bus: &mut B) -> Result<(), B::Error> {
    bus.reset().await;
    bus.command(0x01, &[]).await?;
    bus.delay_ms(150).await;
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
                0xd0, 0x04, 0x0d, 0x11, 0x13, 0x2b, 0x3f, 0x54, 0x4c, 0x18, 0x0d, 0x0b, 0x1f, 0x23,
            ],
        ),
        (
            0xe1,
            &[
                0xd0, 0x04, 0x0c, 0x11, 0x13, 0x2c, 0x3f, 0x44, 0x51, 0x2f, 0x1f, 0x1f, 0x20, 0x23,
            ],
        ),
        (0xb0, &[0x00, 0xf0]),
        (0xb1, &[0xcd, 0x08, 0x14]),
        (0x21, &[]),
    ] {
        bus.command(command, data).await?;
    }
    bus.command(0x11, &[]).await?;
    bus.delay_ms(120).await;
    bus.command(0x13, &[]).await?;
    bus.command(0x29, &[]).await?;
    bus.delay_ms(20).await;
    Ok(())
}

pub const STRIP_ROWS: usize = 4;

pub struct GraphicsMemory {
    assigned: crate::lvgl::Strip<{ crate::lvgl::TFT_WIDTH * STRIP_ROWS }>,
    next: crate::lvgl::Strip<{ crate::lvgl::TFT_WIDTH * STRIP_ROWS }>,
}

impl GraphicsMemory {
    pub const EMPTY: Self = Self {
        assigned: crate::lvgl::Strip::EMPTY,
        next: crate::lvgl::Strip::EMPTY,
    };
}

pub struct GraphicsPanel<'a, B> {
    bus: B,
    memory: &'a mut GraphicsMemory,
}

impl<'a, B: PanelBus> GraphicsPanel<'a, B> {
    pub fn new(bus: B, memory: &'a mut GraphicsMemory) -> Self {
        Self { bus, memory }
    }

    pub async fn initialize(&mut self) -> Result<(), B::Error> {
        initialize(&mut self.bus).await?;
        self.bus.command(0x36, &[0x60]).await
    }

    pub async fn present(
        &mut self,
        previous: Option<&crate::Scene>,
        scene: &crate::Scene,
    ) -> Result<(), B::Error> {
        use crate::lvgl::{Profile, TFT_HEIGHT, TFT_WIDTH, same_band};

        for top in (0..TFT_HEIGHT).step_by(STRIP_ROWS) {
            let rows = top..top + STRIP_ROWS;
            if previous.is_some_and(|previous| same_band(previous, scene, rows.clone())) {
                continue;
            }
            let next = self
                .memory
                .next
                .render(Profile::Tft, scene, rows.clone())
                .unwrap();
            let columns = match previous {
                Some(previous) => {
                    let assigned = self
                        .memory
                        .assigned
                        .render(Profile::Tft, previous, rows.clone())
                        .unwrap();
                    changed_columns(assigned, next, TFT_WIDTH)
                }
                None => 0..TFT_WIDTH,
            };
            if columns.is_empty() {
                continue;
            }
            let offset = 20;
            let [left_hi, left_lo] = ((columns.start + offset) as u16).to_be_bytes();
            let [right_hi, right_lo] = ((columns.end - 1 + offset) as u16).to_be_bytes();
            self.bus
                .command(0x2a, &[left_hi, left_lo, right_hi, right_lo])
                .await?;
            self.bus
                .command(0x2b, &[0, top as u8, 0, (rows.end - 1) as u8])
                .await?;
            self.bus.command(0x2c, &[]).await?;
            let stride = columns.len();
            for row in 0..STRIP_ROWS {
                next.copy_within(
                    row * TFT_WIDTH + columns.start..row * TFT_WIDTH + columns.end,
                    row * stride,
                );
            }
            let pixels = &mut next[..stride * STRIP_ROWS];
            for pixel in pixels.iter_mut() {
                *pixel = pixel.to_be();
            }
            let bytes = unsafe {
                core::slice::from_raw_parts(pixels.as_ptr().cast(), core::mem::size_of_val(pixels))
            };
            self.bus.data(bytes).await?;
        }
        Ok(())
    }

    pub fn into_inner(self) -> B {
        self.bus
    }
}

fn changed_columns(before: &[u16], after: &[u16], width: usize) -> core::ops::Range<usize> {
    let mut left = width;
    let mut right = 0;
    for (index, (before, after)) in before.iter().zip(after).enumerate() {
        if before != after {
            let x = index % width;
            left = left.min(x);
            right = right.max(x + 1);
        }
    }
    left.min(right)..right
}
