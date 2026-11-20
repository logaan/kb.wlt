#![cfg(feature = "lvgl")]

use kb_display::{
    Scene,
    lvgl::{MIP_WIDTH, Profile, Strip, TFT_WIDTH, same_band},
    toucan_display::{ROW_BYTES, TRANSFER_BYTES, encode_scene},
};

#[test]
fn rejects_invalid_draw_commands_before_the_panel_sees_them() {
    for bytes in [
        &[1][..],
        &[1, 0, 0, 0, 0, 1, 0, 1, 0, 10, 0],
        &[3, 0, 0, 0, 0, 1, 0, 1, 0xff],
        &[4, 0, 0, 0, 0, 1, 0, 1, 0, 0, 2],
        &[5, 0, 0, 0, 0, 16, 0, 16, 0, 5, 0],
        &[6, 16, 0, 16, 0, 8, 0, 104, 1, 5, 3, 90],
        &[6, 16, 0, 16, 0, 8, 0, 225, 0, 5, 3],
    ] {
        assert!(Scene::from_bytes(bytes).is_none());
    }
}

#[test]
fn native_round_strokes_clip_consistently_and_survive_mip_quantization() {
    let scene = Scene::from_bytes(&[
        6, 32, 0, 31, 0, 24, 0, 225, 0, 5, 3, 90, 6, 32, 0, 31, 0, 16, 0, 225, 0, 5, 3, 90, 5, 48,
        0, 7, 0, 64, 0, 23, 0, 5, 3, 5, 64, 0, 7, 0, 48, 0, 23, 0, 5, 3,
    ])
    .unwrap();
    assert!(!same_band(&scene, &Scene::EMPTY, 7..8));
    assert!(same_band(&scene, &Scene::EMPTY, 56..64));
    let mut strip = Strip::<{ TFT_WIDTH * 32 }>::EMPTY;
    for profile in [Profile::Tft, Profile::Mip] {
        let width = profile.dimensions().0;
        let pixels = strip.render(profile, &scene, 0..32).unwrap().to_vec();
        for top in (0..32).step_by(8) {
            assert_eq!(
                strip.render(profile, &scene, top..top + 8).unwrap(),
                &pixels[top * width..(top + 8) * width]
            );
        }
        let background = pixels[0];
        let pixel = |x: usize, y: usize| pixels[y * width + x];
        assert_ne!(pixel(32, 8), background);
        assert_eq!(pixel(32, 12), background);
        assert_ne!(pixel(32, 16), background);
        for y in 7..=23 {
            for x in [48 + y - 7, 64 - y + 7] {
                assert!(
                    (x - 1..=x + 1).any(|x| pixel(x, y) != background),
                    "{x},{y} background={background}"
                );
            }
        }
        let shades = |columns: core::ops::Range<usize>| -> std::collections::BTreeSet<_> {
            pixels
                .chunks_exact(width)
                .flat_map(|row| &row[columns.clone()])
                .copied()
                .collect()
        };
        match profile {
            Profile::Tft => {
                assert!(shades(12..46).len() > 2);
                assert!(shades(46..70).len() > 2);
            }
            Profile::Mip => assert_eq!(shades(12..70), [0, u16::MAX].into()),
        }
    }
}

#[test]
fn drawing_arcs_and_labels_through_repeated_frames_keeps_the_draw_heap_available() {
    let commands = [
        6, 32, 0, 31, 0, 24, 0, 225, 0, 5, 3, 90, 6, 32, 0, 31, 0, 17, 0, 225, 0, 5, 3, 90, 6, 32,
        0, 31, 0, 10, 0, 225, 0, 5, 3, 90, 3, 62, 0, 5, 0, 1, 2, 3, b'8', b'0', b'%',
    ];
    let scene = Scene::from_bytes(&commands).unwrap();
    let mut strip = Strip::<{ TFT_WIDTH * 8 }>::EMPTY;
    let expected = strip.render(Profile::Tft, &scene, 16..24).unwrap().to_vec();
    for _ in 0..120 {
        for top in (0..48).step_by(8) {
            assert!(strip.render(Profile::Mip, &scene, top..top + 8).is_some());
            let frame = strip.render(Profile::Tft, &scene, top..top + 8).unwrap();
            if top == 16 {
                assert_eq!(frame, expected);
            }
        }
    }
}

#[test]
fn renders_labels_and_both_dot_styles_across_strip_boundaries() {
    let scene = Scene::from_bytes(&[
        4, 0, 0, 0, 0, 16, 0, 20, 0, 0x01, 3, 0x42, 4, 16, 0, 0, 0, 16, 0, 20, 0, 0x01, 0x84, 0x42,
        3, 32, 0, 0, 0, 1, 3, 3, b'F', b'o', b'x',
    ])
    .unwrap();
    let mut strip = Strip::<{ TFT_WIDTH * 8 }>::EMPTY;
    let header = strip.render(Profile::Tft, &scene, 0..8).unwrap();
    assert_ne!(header[2 * TFT_WIDTH + 4], header[2 * TFT_WIDTH + 20]);
    assert!(
        header[32..TFT_WIDTH]
            .iter()
            .all(|&pixel| pixel == header[40])
    );
    assert!((1..8).any(|row| {
        header[row * TFT_WIDTH + 32..(row + 1) * TFT_WIDTH]
            .iter()
            .any(|&pixel| pixel != header[40])
    }));

    assert!(!same_band(&scene, &Scene::EMPTY, 16..20));
    assert!(same_band(&scene, &Scene::EMPTY, 30..34));
    let footer = strip.render(Profile::Tft, &scene, 16..24).unwrap();
    let normal = footer[3 * TFT_WIDTH + 1];
    let underlined = footer[3 * TFT_WIDTH + 17];
    let background = footer[3 * TFT_WIDTH + 40];
    assert_eq!(normal, background);
    assert_ne!(underlined, background);
}

#[test]
fn encodes_mip_pixels_in_panel_bit_order_with_vcom() {
    let scene = Scene::from_bytes(&[1, 1, 0, 2, 0, 3, 0, 1, 0, 1, 0]).unwrap();
    let mut strip = Strip::<{ MIP_WIDTH * 4 }>::EMPTY;
    let pixels = strip.render(Profile::Mip, &scene, 0..4).unwrap();
    assert_eq!(
        &pixels[2 * MIP_WIDTH..2 * MIP_WIDTH + 8],
        &[0xffff, 0, 0, 0, 0xffff, 0xffff, 0xffff, 0xffff]
    );

    let mut transfer = [0; TRANSFER_BYTES];
    encode_scene(&scene, true, &mut transfer);
    assert_eq!(transfer[0], 3);
    let line = 1 + 2 * (ROW_BYTES + 2);
    assert_eq!(&transfer[line..line + 2], &[3, 0xf1]);
    assert_eq!(transfer[line + ROW_BYTES + 1], 0);
}

#[test]
fn row_major_mask_can_draw_a_three_by_two_grid() {
    let scene = Scene::from_bytes(&[4, 0, 0, 0, 0, 24, 0, 16, 0, 0b100001, 3, 0x23]).unwrap();
    let mut strip = Strip::<{ TFT_WIDTH * 4 }>::EMPTY;
    let background = strip.render(Profile::Tft, &Scene::EMPTY, 2..6).unwrap()[0];
    let upper = strip.render(Profile::Tft, &scene, 2..6).unwrap();
    let on = upper[2 * TFT_WIDTH + 4];
    assert_ne!(on, background);
    assert_eq!(upper[2 * TFT_WIDTH + 20], background);
    let lower = strip.render(Profile::Tft, &scene, 10..14).unwrap();
    assert_eq!(lower[2 * TFT_WIDTH + 4], background);
    assert_eq!(lower[2 * TFT_WIDTH + 20], on);
}

#[test]
fn body_text_has_the_designed_visible_height_on_each_panel() {
    let scene = Scene::from_bytes(&[3, 0, 0, 0, 0, 1, 0, 1, b'H']).unwrap();
    let mut strip = Strip::<{ TFT_WIDTH * 4 }>::EMPTY;
    for (profile, minimum) in [(Profile::Tft, 20), (Profile::Mip, 14)] {
        let width = profile.dimensions().0;
        let background = strip.render(profile, &Scene::EMPTY, 0..4).unwrap()[0];
        let mut ink_rows = 0;
        for y in (0..32).step_by(4) {
            ink_rows += strip
                .render(profile, &scene, y..y + 4)
                .unwrap()
                .chunks_exact(width)
                .filter(|row| row.iter().any(|&pixel| pixel != background))
                .count();
        }
        assert!(ink_rows >= minimum, "only {ink_rows} visible rows");
    }
}

#[test]
fn an_expanded_dot_grid_adds_one_native_pixel_of_clearance_in_both_axes() {
    let mut strip = Strip::<{ TFT_WIDTH * 8 }>::EMPTY;
    for (profile, previous_pitch) in [(Profile::Tft, 27), (Profile::Mip, 18)] {
        let mut gaps = Vec::new();
        for extra_gap in [false, true] {
            let pitch = previous_pitch + u8::from(extra_gap);
            let scene = Scene::from_bytes(&[
                4,
                0,
                0,
                0,
                0,
                pitch * 2,
                0,
                pitch * 4,
                0,
                255,
                3 | (u8::from(extra_gap) << 6),
                0x42,
            ])
            .unwrap();
            let background = strip.render(profile, &Scene::EMPTY, 0..1).unwrap()[0];
            let center = usize::from(pitch / 2);
            let horizontal = strip.render(profile, &scene, center..center + 1).unwrap();
            let gap = |pixels: &[u16], pitch: usize| {
                pixels[..2 * pitch]
                    .iter()
                    .filter(|&&pixel| pixel == background)
                    .count()
                    - pixels[..pitch]
                        .iter()
                        .take_while(|&&pixel| pixel == background)
                        .count()
                    - pixels[pitch..2 * pitch]
                        .iter()
                        .rev()
                        .take_while(|&&pixel| pixel == background)
                        .count()
            };
            let h = gap(horizontal, usize::from(pitch));
            let vertical: Vec<_> = (0..usize::from(pitch) * 2)
                .map(|y| strip.render(profile, &scene, y..y + 1).unwrap()[center])
                .collect();
            gaps.push((h, gap(&vertical, usize::from(pitch))));
        }
        assert_eq!(gaps, [(1, 1), (2, 2)]);
    }
}
