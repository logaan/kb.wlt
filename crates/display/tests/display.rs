use embedded_graphics::{draw_target::DrawTarget, pixelcolor::BinaryColor, prelude::*};
use embedded_hal_async::i2c::{ErrorType, I2c, Operation};
use kb_display::{Frame, Ssd1306, Status, Transport, render};

#[derive(Debug, Default)]
struct Bus {
    writes: Vec<(u8, Vec<u8>)>,
    fail_after: Option<usize>,
}

impl ErrorType for Bus {
    type Error = embedded_hal_async::i2c::ErrorKind;
}

impl I2c for Bus {
    async fn transaction(
        &mut self,
        address: u8,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        for operation in operations {
            if self.fail_after == Some(self.writes.len()) {
                return Err(Self::Error::Other);
            }
            match operation {
                Operation::Write(bytes) => self.writes.push((address, bytes.to_vec())),
                Operation::Read(_) => panic!("display must only write"),
            }
        }
        Ok(())
    }
}

#[test]
fn framebuffer_uses_ssd1306_page_order_and_clips() {
    let mut frame = Frame::default();
    frame
        .draw_iter([
            Pixel(Point::new(0, 0), BinaryColor::On),
            Pixel(Point::new(127, 31), BinaryColor::On),
            Pixel(Point::new(-1, 1), BinaryColor::On),
            Pixel(Point::new(128, 1), BinaryColor::On),
            Pixel(Point::new(1, 32), BinaryColor::On),
        ])
        .unwrap();
    assert_eq!(frame.bytes()[0], 1);
    assert_eq!(frame.bytes()[511], 128);
    assert_eq!(
        frame
            .bytes()
            .iter()
            .map(|byte| byte.count_ones())
            .sum::<u32>(),
        2
    );
    frame
        .draw_iter([Pixel(Point::new(0, 0), BinaryColor::Off)])
        .unwrap();
    assert_eq!(frame.bytes()[0], 0);
}

#[test]
fn initializes_geometry_and_transmits_complete_frame() {
    futures::executor::block_on(async {
        let frame = render(Status {
            connected: true,
            held: 0x101,
            chord: 3,
            last_usage: 4,
            ..Status::default()
        });
        let mut display = Ssd1306::new(Bus::default(), 0x3c);
        display.initialize().await.unwrap();
        display.present(&frame).await.unwrap();
        let bus = display.into_inner();
        assert!(bus.writes.iter().all(|(address, _)| *address == 0x3c));
        assert_eq!(bus.writes[0].1.last(), Some(&0xaf));
        assert!(bus.writes[0].1.windows(2).any(|pair| pair == [0xa8, 31]));
        assert_eq!(bus.writes[1].1, [0, 0x21, 0, 127, 0x22, 0, 3]);
        let sent: Vec<u8> = bus.writes[2..]
            .iter()
            .flat_map(|(_, bytes)| {
                assert_eq!(bytes[0], 0x40);
                bytes[1..].iter().copied()
            })
            .collect();
        assert_eq!(sent, frame.bytes());
    });
}

#[test]
fn i2c_failures_stop_frame_transfer() {
    futures::executor::block_on(async {
        let mut display = Ssd1306::new(
            Bus {
                fail_after: Some(2),
                ..Bus::default()
            },
            0x3c,
        );
        assert!(display.present(&Frame::default()).await.is_err());
        assert_eq!(display.into_inner().writes.len(), 2);
    });
}

#[test]
fn status_changes_have_distinct_pixels() {
    let idle = render(Status::default());
    for status in [
        Status {
            connected: true,
            ..Status::default()
        },
        Status {
            transport: Transport::Bluetooth,
            ..Status::default()
        },
        Status {
            held: 0x100,
            chord: 0x40,
            ..Status::default()
        },
        Status {
            last_usage: 4,
            last_modifiers: 2,
            ..Status::default()
        },
    ] {
        assert!(render(status).bytes() != idle.bytes());
    }
}

#[test]
fn fault_state_replaces_live_keyboard_status() {
    let fault = Status {
        faulted: true,
        ..Status::default()
    };
    let live = Status {
        connected: true,
        ..Status::default()
    };
    assert!(render(fault).bytes() != render(live).bytes());
    assert_eq!(
        render(fault).bytes(),
        render(Status {
            connected: true,
            held: 0x1ff,
            chord: 0x1ff,
            last_usage: 4,
            keyboard: Some(kb_display::KeyboardStatus {
                active_keys: 42,
                layer: 1
            }),
            ..fault
        })
        .bytes()
    );
    assert!(
        kb_display::nice_view::render(fault).bytes() != kb_display::nice_view::render(live).bytes()
    );
    let mut row = [0; 480];
    kb_display::st7789::render_row(&render(fault), fault, 0, &mut row);
    assert_eq!(&row[..2], &0x7800u16.to_be_bytes());
}
