use kb_display::{
    Status, Transport, render,
    st7789::{PanelBus, St7789, render_row},
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
fn uses_portrait_240_by_280_with_twenty_row_offset_and_rgb565() {
    futures::executor::block_on(async {
        let mut panel = St7789::new(Bus::default());
        panel.initialize().await.unwrap();
        panel.present(Status::default()).await.unwrap();
        let bus = panel.into_inner();
        assert_eq!(bus.resets, 1);
        assert_eq!(bus.waits, [150, 120, 20]);
        assert!(bus.commands.contains(&(0x3a, vec![0x05])));
        assert!(bus.commands.contains(&(0x2a, vec![0, 0, 0, 239])));
        assert!(bus.commands.contains(&(0x2b, vec![0, 20, 1, 43])));
        assert_eq!(bus.pixels.len(), 240 * 280 * 2);
    });
}

#[test]
fn unlocks_extended_registers_and_configures_panel_before_sleep_out() {
    futures::executor::block_on(async {
        let mut panel = St7789::new(Bus::default());
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
            assert!(bus.commands[3..bus.commands.len() - 3].contains(&command));
        }
        assert_eq!(
            &bus.commands[bus.commands.len() - 3..],
            &[(0x11, vec![]), (0x13, vec![]), (0x29, vec![])]
        );
        assert_eq!(bus.waits, [150, 120, 20]);
    });
}

#[test]
fn colors_distinguish_accumulated_dot_eight_from_held_space() {
    let mut row = [0; 480];
    for (held, chord, dot_eight, space) in [
        (0x100, 1, 0x0000u16, 0xffe0u16),
        (0x80, 0x81, 0x07e0, 0x2104),
        (0, 0x81, 0x07e0, 0x2104),
        (0x180, 0x181, 0x07e0, 0xffe0),
    ] {
        let status = Status {
            connected: true,
            transport: Transport::Split,
            held,
            chord,
            ..Status::default()
        };
        let frame = render(status);
        render_row(&frame, status, 134, &mut row);
        assert_eq!(&row[184..186], &0x07e0u16.to_be_bytes());
        render_row(&frame, status, 230, &mut row);
        assert_eq!(&row[296..298], &dot_eight.to_be_bytes());
        render_row(&frame, status, 245, &mut row);
        assert_eq!(&row[240..242], &space.to_be_bytes());
    }
}

#[test]
fn keyboard_uses_clockwise_landscape_and_restores_portrait_for_other_views() {
    futures::executor::block_on(async {
        let mut panel = St7789::new(Bus::default());
        panel
            .present(Status {
                keyboard: Some(kb_display::KeyboardStatus::default()),
                ..Status::default()
            })
            .await
            .unwrap();
        panel.present(Status::default()).await.unwrap();
        let bus = panel.into_inner();
        assert_eq!(
            bus.commands,
            [
                (0x36, vec![0x60]),
                (0x2a, vec![0, 20, 1, 43]),
                (0x2b, vec![0, 0, 0, 239]),
                (0x2c, vec![]),
                (0x36, vec![0x00]),
                (0x2a, vec![0, 0, 0, 239]),
                (0x2b, vec![0, 20, 1, 43]),
                (0x2c, vec![]),
            ]
        );
        assert_eq!(bus.pixels.len(), 2 * 240 * 280 * 2);
    });
}

#[test]
fn chord_updates_reconstruct_the_full_frame_within_a_30_hz_wire_budget() {
    use kb_core::KeyStroke;
    use kb_display::{
        KeyboardStatus,
        keyboard::{self, LiveBraille, release_label},
    };
    let idle = Status {
        keyboard: Some(KeyboardStatus {
            layer: keyboard::BRAILLE_LAYER,
            active_keys: 0,
        }),
        braille: Some(LiveBraille::default()),
        connected: true,
        ..Status::default()
    };
    let pressed = Status {
        chord: 1,
        braille: Some(LiveBraille {
            release: release_label(&[KeyStroke::new(4, 0)], false, false),
            ..Default::default()
        }),
        ..idle
    };
    let buffered = Status {
        braille: Some(LiveBraille {
            buffered: [1, 0, 0, 0, 0, 0, 0],
            ..Default::default()
        }),
        ..idle
    };
    for (before, after, budget) in [
        (idle, pressed, 4_000_000 / 8 / 30),
        (pressed, buffered, 4_000_000 / 8 / 30),
        (pressed, pressed, 0),
        (
            pressed,
            Status {
                connected: false,
                ..pressed
            },
            280 * 240 * 2,
        ),
        (
            pressed,
            Status {
                faulted: true,
                ..pressed
            },
            280 * 240 * 2 + 1000,
        ),
    ] {
        let bus = futures::executor::block_on(async {
            let mut panel = St7789::new(Bus::default());
            panel.present(before).await.unwrap();
            panel.present(after).await.unwrap();
            panel.into_inner()
        });
        let size = keyboard::WIDTH * keyboard::HEIGHT * 2;
        let mut image = bus.pixels[..size].to_vec();
        let mut data = &bus.pixels[size..];
        let bytes = data.len() + (bus.commands.len() - 4) / 3 * 11;
        assert!(bytes <= budget, "{bytes} exceeds {budget}");
        println!("dashboard update: {bytes} wire bytes");
        for window in bus.commands[4..].chunks_exact(3) {
            let bounds = |b: &[u8]| {
                (
                    usize::from(u16::from_be_bytes([b[0], b[1]])),
                    usize::from(u16::from_be_bytes([b[2], b[3]])),
                )
            };
            let (l, r) = bounds(&window[0].1);
            let (t, b) = bounds(&window[1].1);
            let len = (r - l + 1) * 2;
            for y in t..=b {
                let start = (y * keyboard::WIDTH + l - 20) * 2;
                image[start..start + len].copy_from_slice(&data[..len]);
                data = &data[len..];
            }
        }
        assert!(data.is_empty());
        let frame = keyboard::render(after);
        for (y, line) in image.chunks_exact(keyboard::WIDTH * 2).enumerate() {
            let mut expected = [0; keyboard::WIDTH * 2];
            keyboard::render_row(&frame, after, y, &mut expected);
            assert_eq!(line, expected, "row {y}");
        }
    }
}

#[test]
fn returning_from_a_menu_invalidates_the_dashboard_cache() {
    futures::executor::block_on(async {
        let mut panel = St7789::new(Bus::default());
        let status = Status {
            keyboard: Some(Default::default()),
            ..Default::default()
        };
        panel.present(status).await.unwrap();
        panel.present_menu(&Default::default()).await.unwrap();
        panel.present(status).await.unwrap();
        assert_eq!(panel.into_inner().pixels.len(), 3 * 280 * 240 * 2);
    });
}

#[test]
fn bus_errors_are_propagated() {
    futures::executor::block_on(async {
        let mut panel = St7789::new(Bus {
            fail: true,
            ..Bus::default()
        });
        assert!(panel.initialize().await.is_err());
        assert!(panel.present(Status::default()).await.is_err());
    });
}

#[test]
fn reference_page_uses_the_same_panel_geometry_and_reports_bus_errors() {
    futures::executor::block_on(async {
        let mut panel = St7789::new(Bus::default());
        panel
            .present_reference(&kb_core::reference::Page::default())
            .await
            .unwrap();
        let bus = panel.into_inner();
        assert_eq!(bus.pixels.len(), 240 * 280 * 2);
        assert!(bus.commands.contains(&(0x2b, vec![0, 20, 1, 43])));
        let mut failed = St7789::new(Bus {
            fail: true,
            ..Bus::default()
        });
        assert!(
            failed
                .present_reference(&kb_core::reference::Page::default())
                .await
                .is_err()
        );
    });
}
