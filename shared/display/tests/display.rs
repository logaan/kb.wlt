use embedded_graphics::{draw_target::DrawTarget, pixelcolor::BinaryColor, prelude::*};
use kb_display::{Frame, Status, Transport, render};

#[test]
fn monochrome_framebuffer_packs_pixels_and_clips() {
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

#[test]
fn unsaved_pairing_warning_keeps_status_and_yields_to_component_faults() {
    let live = Status {
        connected: true,
        ..Status::default()
    };
    let warning = Status {
        pairing_unsaved: true,
        ..live
    };
    assert!(render(warning).bytes() != render(live).bytes());
    assert!(
        kb_display::nice_view::render(warning).bytes()
            != kb_display::nice_view::render(live).bytes()
    );
    let fault = Status {
        faulted: true,
        ..live
    };
    assert_eq!(
        render(fault).bytes(),
        render(Status {
            pairing_unsaved: true,
            ..fault
        })
        .bytes()
    );
    let mut row = [0; 480];
    kb_display::st7789::render_row(&render(warning), warning, 0, &mut row);
    assert_eq!(&row[..2], &0x6200u16.to_be_bytes());
}
