use kb_display::{
    KeyboardStatus, Status,
    keyboard::{HEIGHT, WIDTH, render, render_row},
};

fn pixels(status: Status) -> Vec<u8> {
    let frame = render(status);
    let mut pixels = Vec::new();
    for y in 0..HEIGHT {
        let mut row = [0; WIDTH * 2];
        render_row(&frame, status, y, &mut row);
        pixels.extend_from_slice(&row);
    }
    pixels
}

#[test]
fn keyboard_dashboard_fills_landscape_and_updates_each_status_field() {
    let status = Status {
        connected: true,
        keyboard: Some(KeyboardStatus {
            active_keys: 3,
            layer: 1,
        }),
        last_usage: 4,
        last_modifiers: 2,
        ..Status::default()
    };
    let baseline = pixels(status);
    assert_eq!(baseline.len(), 280 * 240 * 2);
    assert_eq!(&baseline[..2], &0x0184u16.to_be_bytes());
    assert_eq!(&baseline[baseline.len() - 2..], &[0, 0]);
    for changed in [
        Status {
            connected: false,
            ..status
        },
        Status {
            keyboard: Some(KeyboardStatus {
                active_keys: 42,
                layer: 1,
            }),
            ..status
        },
        Status {
            keyboard: Some(KeyboardStatus {
                active_keys: 3,
                layer: 2,
            }),
            ..status
        },
        Status {
            last_usage: 0x28,
            ..status
        },
        Status {
            last_modifiers: 0x80,
            ..status
        },
        Status {
            pairing_unsaved: true,
            ..status
        },
    ] {
        assert_ne!(baseline, pixels(changed));
    }
}

#[test]
fn header_text_stays_inside_rounded_corner_inset() {
    for pairing_unsaved in [false, true] {
        let image = pixels(Status {
            connected: true,
            pairing_unsaved,
            ..Status::default()
        });
        let mut title_pixels = 0;
        for y in 0..34 {
            for x in 0..WIDTH {
                let offset = (y * WIDTH + x) * 2;
                if image[offset..offset + 2] == [0xff, 0xff] {
                    assert!(x >= 16 && x < WIDTH - 16);
                    assert!(y >= 16);
                    title_pixels += 1;
                }
            }
        }
        assert!(title_pixels > 0);
    }
}

#[test]
fn faults_replace_live_values_instead_of_showing_stale_keys() {
    let fault = Status {
        faulted: true,
        ..Status::default()
    };
    assert_eq!(
        pixels(fault),
        pixels(Status {
            keyboard: Some(KeyboardStatus {
                active_keys: 42,
                layer: 255
            }),
            last_usage: 0xff,
            last_modifiers: 0xff,
            ..fault
        })
    );
}
