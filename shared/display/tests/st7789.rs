#![cfg(feature = "lvgl")]

use kb_display::{
    Scene,
    lvgl::{Profile, Strip, TFT_HEIGHT, TFT_WIDTH},
    st7789::{GraphicsMemory, GraphicsPanel, PanelBus, STRIP_ROWS},
};

#[derive(Default)]
struct Bus {
    commands: Vec<(u8, Vec<u8>)>,
    pixels: Vec<u8>,
    waits: Vec<u32>,
    resets: usize,
    fail: bool,
}

impl PanelBus for Bus {
    type Error = ();

    async fn reset(&mut self) {
        self.resets += 1;
    }

    async fn delay_ms(&mut self, milliseconds: u32) {
        self.waits.push(milliseconds);
    }

    async fn command(&mut self, command: u8, data: &[u8]) -> Result<(), ()> {
        if self.fail {
            return Err(());
        }
        self.commands.push((command, data.to_vec()));
        Ok(())
    }

    async fn data(&mut self, data: &[u8]) -> Result<(), ()> {
        if self.fail {
            return Err(());
        }
        self.pixels.extend_from_slice(data);
        Ok(())
    }
}

#[test]
fn initializes_landscape_rgb565_with_twenty_column_offset() {
    futures::executor::block_on(async {
        let mut memory = GraphicsMemory::EMPTY;
        let mut panel = GraphicsPanel::new(Bus::default(), &mut memory);
        panel.initialize().await.unwrap();
        panel.present(None, &Scene::EMPTY).await.unwrap();
        let bus = panel.into_inner();
        assert_eq!(bus.resets, 1);
        assert_eq!(bus.waits, [150, 120, 20]);
        assert_eq!((TFT_WIDTH, TFT_HEIGHT), (280, 240));
        assert!(bus.commands.contains(&(0x3a, vec![0x05])));
        assert!(bus.commands.contains(&(0x36, vec![0x60])));
        assert!(bus.commands.contains(&(0x2a, vec![0, 20, 1, 43])));
        assert!(bus.commands.contains(&(0x2b, vec![0, 0, 0, 3])));
        assert!(bus.commands.contains(&(0x2b, vec![0, 236, 0, 239])));
        assert_eq!(bus.pixels.len(), 280 * 240 * 2);
    });
}

#[test]
fn unlocks_extended_registers_and_configures_panel_before_sleep_out() {
    futures::executor::block_on(async {
        let mut memory = GraphicsMemory::EMPTY;
        let mut panel = GraphicsPanel::new(Bus::default(), &mut memory);
        panel.initialize().await.unwrap();
        let bus = panel.into_inner();
        assert_eq!(bus.commands[0], (0x01, vec![]));
        assert_eq!(bus.commands[1], (0x28, vec![]));
        assert_eq!(bus.commands[2], (0xdf, vec![0x5a, 0x69, 0x02, 0x01]));
        for command in [
            (0xba, vec![0x00]),
            (0x26, vec![0x01]),
            (0xb0, vec![0x00, 0xf0]),
            (0xb1, vec![0xcd, 0x08, 0x14]),
        ] {
            assert!(bus.commands[3..bus.commands.len() - 4].contains(&command));
        }
        assert_eq!(
            &bus.commands[bus.commands.len() - 4..],
            &[
                (0x11, vec![]),
                (0x13, vec![]),
                (0x29, vec![]),
                (0x36, vec![0x60])
            ]
        );
    });
}

fn apply_updates(image: &mut [u8], commands: &[(u8, Vec<u8>)], mut data: &[u8]) {
    for window in commands.chunks_exact(3) {
        assert_eq!([window[0].0, window[1].0, window[2].0], [0x2a, 0x2b, 0x2c]);
        let bounds = |bytes: &[u8]| {
            (
                usize::from(u16::from_be_bytes([bytes[0], bytes[1]])),
                usize::from(u16::from_be_bytes([bytes[2], bytes[3]])),
            )
        };
        let (left, right) = bounds(&window[0].1);
        let (top, bottom) = bounds(&window[1].1);
        let stride = (right - left + 1) * 2;
        for row in top..=bottom {
            let start = (row * TFT_WIDTH + left - 20) * 2;
            image[start..start + stride].copy_from_slice(&data[..stride]);
            data = &data[stride..];
        }
    }
    assert!(data.is_empty());
}

#[test]
fn chord_updates_reconstruct_the_full_frame_within_a_30_hz_wire_budget() {
    let idle = Scene::from_bytes(&[4, 113, 0, 6, 0, 54, 0, 108, 0, 0, 3, 0x42]).unwrap();
    let pressed = Scene::from_bytes(&[
        4, 113, 0, 6, 0, 54, 0, 108, 0, 1, 3, 0x42, 3, 122, 0, 170, 0, 1, 2, 1, b'a',
    ])
    .unwrap();
    let buffered = Scene::from_bytes(&[
        4, 113, 0, 6, 0, 54, 0, 108, 0, 0, 3, 0x42, 4, 132, 0, 124, 0, 16, 0, 24, 0, 1, 3, 0x42,
    ])
    .unwrap();
    let frame_bytes = TFT_WIDTH * TFT_HEIGHT * 2;
    let full_frame_commands = TFT_HEIGHT / STRIP_ROWS * 3;
    for (before, after, budget) in [
        (&idle, &pressed, 4_000_000 / 8 / 30),
        (&pressed, &buffered, 4_000_000 / 8 / 30),
        (&pressed, &pressed, 0),
    ] {
        let bus = futures::executor::block_on(async {
            let mut memory = GraphicsMemory::EMPTY;
            let mut panel = GraphicsPanel::new(Bus::default(), &mut memory);
            panel.present(None, before).await.unwrap();
            panel.present(Some(before), after).await.unwrap();
            panel.into_inner()
        });
        let mut image = bus.pixels[..frame_bytes].to_vec();
        let data = &bus.pixels[frame_bytes..];
        let commands = &bus.commands[full_frame_commands..];
        let bytes = data.len() + commands.len() / 3 * 11;
        assert!(bytes <= budget, "{bytes} exceeds {budget}");
        apply_updates(&mut image, commands, data);
        let mut strip = Strip::<{ TFT_WIDTH * STRIP_ROWS }>::EMPTY;
        for (band, actual) in image.chunks_exact(TFT_WIDTH * STRIP_ROWS * 2).enumerate() {
            let top = band * STRIP_ROWS;
            let expected = strip
                .render(Profile::Tft, after, top..top + STRIP_ROWS)
                .unwrap();
            for (bytes, pixel) in actual.chunks_exact(2).zip(expected) {
                assert_eq!(bytes, pixel.to_be_bytes(), "strip {band}");
            }
        }
    }
}

#[test]
fn bus_errors_are_propagated() {
    futures::executor::block_on(async {
        let mut memory = GraphicsMemory::EMPTY;
        let mut panel = GraphicsPanel::new(
            Bus {
                fail: true,
                ..Bus::default()
            },
            &mut memory,
        );
        assert!(panel.initialize().await.is_err());
        assert!(panel.present(None, &Scene::EMPTY).await.is_err());
    });
}
