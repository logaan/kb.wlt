use embedded_graphics::{draw_target::DrawTarget, pixelcolor::BinaryColor, prelude::*};
use kb_display::nice_view::{Frame, TRANSFER_BYTES, clear_command, encode_frame, idle_command};

#[test]
fn portrait_pixels_rotate_into_lsb_first_white_background() {
    let mut frame = Frame::default();
    frame
        .draw_iter([
            Pixel(Point::new(0, 0), BinaryColor::On),
            Pixel(Point::new(67, 159), BinaryColor::On),
            Pixel(Point::new(-1, 0), BinaryColor::On),
        ])
        .unwrap();
    assert_eq!(frame.bytes()[67 * 20], 0xfe);
    assert_eq!(frame.bytes()[19], 0x7f);
    assert_eq!(
        frame
            .bytes()
            .iter()
            .map(|byte| byte.count_zeros())
            .sum::<u32>(),
        2
    );
}

#[test]
fn each_line_has_one_based_address_and_dummy_byte() {
    let mut transfer = [0; TRANSFER_BYTES];
    encode_frame(&Frame::default(), true, &mut transfer);
    assert_eq!(transfer[0], 3);
    assert_eq!(transfer[TRANSFER_BYTES - 1], 0);
    for (index, line) in transfer[1..TRANSFER_BYTES - 1].chunks_exact(22).enumerate() {
        assert_eq!(line[0], index as u8 + 1);
        assert_eq!(&line[1..21], &[0xff; 20]);
        assert_eq!(line[21], 0);
    }
    assert_eq!(idle_command(false), [0, 0]);
    assert_eq!(idle_command(true), [2, 0]);
    assert_eq!(clear_command(false), [4, 0]);
    assert_eq!(clear_command(true), [6, 0]);
}
