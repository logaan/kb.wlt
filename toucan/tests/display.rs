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
fn keyboard_guide_fills_landscape_and_only_updates_user_facing_state() {
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
                active_keys: 3,
                layer: 2,
            }),
            ..status
        },
        Status {
            pairing_unsaved: true,
            ..status
        },
    ] {
        assert_ne!(baseline, pixels(changed));
    }
    assert_eq!(
        baseline,
        pixels(Status {
            last_usage: 40,
            last_modifiers: 128,
            keyboard: Some(KeyboardStatus {
                layer: 1,
                active_keys: 42
            }),
            ..status
        })
    );
}

#[test]
fn braille_preview_changes_with_the_result_and_buffer_even_if_dots_stay_the_same() {
    use kb_core::KeyStroke;
    use kb_display::keyboard::{LiveBraille, release_label};
    let live = LiveBraille {
        release: release_label(&[KeyStroke::new(4, 0)], false, false),
        ..Default::default()
    };
    assert_eq!(live.release.as_str(), "a");
    let status = Status {
        keyboard: Some(KeyboardStatus {
            layer: 2,
            active_keys: 1,
        }),
        braille: Some(live),
        chord: 1,
        ..Default::default()
    };
    for changed in [
        LiveBraille {
            release: release_label(&[], true, false),
            ..live
        },
        LiveBraille {
            buffered: [1, 0, 0, 0, 0, 0, 0],
            ..live
        },
    ] {
        assert_ne!(
            pixels(status),
            pixels(Status {
                braille: Some(changed),
                ..status
            })
        );
    }
    assert_eq!(release_label(&[], false, true).as_str(), "Menu prefix");
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
