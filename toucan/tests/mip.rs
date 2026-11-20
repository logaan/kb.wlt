use kb_display::{
    Scene,
    toucan_display::{
        FRAME_BYTES, HEIGHT, ROW_BYTES, TRANSFER_BYTES, WIDTH, clear_command, encode_scene,
        idle_command,
    },
};

#[test]
fn scene_pixels_use_native_portrait_geometry_and_lsb_first_packing() {
    let scene = Scene::from_bytes(&[
        1, 0, 0, 0, 0, 1, 0, 1, 0, 1, 0, 1, 143, 0, 167, 0, 1, 0, 1, 0, 1, 0, 1, 144, 0, 0, 0, 1,
        0, 1, 0, 1, 0, 1, 0, 0, 168, 0, 1, 0, 1, 0, 1, 0,
    ])
    .unwrap();
    assert_eq!((WIDTH, HEIGHT, FRAME_BYTES), (144, 168, 3024));
    let mut transfer = [0; TRANSFER_BYTES];
    encode_scene(&scene, false, &mut transfer);
    assert_eq!(transfer[0], 1);
    assert_eq!(transfer[2], 0xfe);
    assert_eq!(transfer[3359], 0x7f);
    let black = transfer[1..TRANSFER_BYTES - 1]
        .chunks_exact(ROW_BYTES + 2)
        .flat_map(|line| &line[1..=ROW_BYTES])
        .map(|byte| byte.count_zeros())
        .sum::<u32>();
    assert_eq!(black, 2);
}

#[test]
fn each_line_has_one_based_address_and_dummy_byte() {
    let mut transfer = [0; TRANSFER_BYTES];
    encode_scene(&Scene::EMPTY, true, &mut transfer);
    assert_eq!(transfer.len(), 3362);
    assert_eq!(transfer[0], 3);
    assert_eq!(transfer[3341], 168);
    assert_eq!(transfer[TRANSFER_BYTES - 1], 0);
    for (index, line) in transfer[1..TRANSFER_BYTES - 1].chunks_exact(20).enumerate() {
        assert_eq!(line[0], index as u8 + 1);
        assert_eq!(&line[1..19], &[0xff; 18]);
        assert_eq!(line[19], 0);
    }
    assert_eq!(idle_command(false), [0, 0]);
    assert_eq!(idle_command(true), [2, 0]);
    assert_eq!(clear_command(false), [4, 0]);
    assert_eq!(clear_command(true), [6, 0]);
}
