pub const WIDTH: usize = 144;
pub const HEIGHT: usize = 168;
pub const ROW_BYTES: usize = WIDTH / 8;
pub const FRAME_BYTES: usize = ROW_BYTES * HEIGHT;
pub const TRANSFER_BYTES: usize = 1 + HEIGHT * (ROW_BYTES + 2) + 1;

pub const fn idle_command(vcom: bool) -> [u8; 2] {
    [if vcom { 2 } else { 0 }, 0]
}
pub const fn clear_command(vcom: bool) -> [u8; 2] {
    [4 | if vcom { 2 } else { 0 }, 0]
}

#[cfg(feature = "lvgl-mip")]
pub fn encode_scene(scene: &crate::Scene, vcom: bool, output: &mut [u8; TRANSFER_BYTES]) {
    use crate::lvgl::{Profile, Strip};

    const ROWS: usize = 4;
    let mut strip = Strip::<{ WIDTH * ROWS }>::EMPTY;
    output[0] = 0x01 | if vcom { 0x02 } else { 0 };
    for top in (0..HEIGHT).step_by(ROWS) {
        let pixels = strip.render(Profile::Mip, scene, top..top + ROWS).unwrap();
        for (offset, row) in pixels.chunks_exact(WIDTH).enumerate() {
            let line = top + offset;
            let start = 1 + line * (ROW_BYTES + 2);
            output[start] = line as u8 + 1;
            for (index, pixels) in row.chunks_exact(8).enumerate() {
                let mut byte = 0;
                for (bit, pixel) in pixels.iter().copied().enumerate() {
                    if pixel != 0 {
                        byte |= 1 << bit;
                    }
                }
                output[start + 1 + index] = byte;
            }
            output[start + 1 + ROW_BYTES] = 0;
        }
    }
    output[TRANSFER_BYTES - 1] = 0;
}
