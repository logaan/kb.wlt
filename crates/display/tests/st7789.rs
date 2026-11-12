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
        assert!(bus.commands.contains(&(0x3a, vec![0x55])));
        assert!(bus.commands.contains(&(0x2a, vec![0, 0, 0, 239])));
        assert!(bus.commands.contains(&(0x2b, vec![0, 20, 1, 43])));
        assert_eq!(bus.pixels.len(), 240 * 280 * 2);
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
