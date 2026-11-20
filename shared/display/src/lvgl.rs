use core::{
    ffi::c_char,
    ops::Range,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::{Scene, scene::Command};

pub const TFT_WIDTH: usize = 280;
pub const TFT_HEIGHT: usize = 240;
pub const MIP_WIDTH: usize = 144;
pub const MIP_HEIGHT: usize = 168;

#[derive(Clone, Copy)]
pub enum Profile {
    #[cfg(feature = "lvgl-tft")]
    Tft,
    #[cfg(feature = "lvgl-mip")]
    Mip,
}

impl Profile {
    pub const fn dimensions(self) -> (usize, usize) {
        match self {
            #[cfg(feature = "lvgl-tft")]
            Self::Tft => (TFT_WIDTH, TFT_HEIGHT),
            #[cfg(feature = "lvgl-mip")]
            Self::Mip => (MIP_WIDTH, MIP_HEIGHT),
        }
    }

    fn monochrome(self) -> bool {
        match self {
            #[cfg(feature = "lvgl-tft")]
            Self::Tft => false,
            #[cfg(feature = "lvgl-mip")]
            Self::Mip => true,
        }
    }

    fn color(self, index: u8) -> u32 {
        #[cfg(feature = "lvgl-tft")]
        const TFT: [u32; 10] = [
            0x101921, 0xf9fbff, 0xb2c1cc, 0x60b9ff, 0xd99cff, 0x57dfab, 0xffc476, 0x213344,
            0x334d65, 0xffffff,
        ];
        #[cfg(feature = "lvgl-mip")]
        const MIP: [u32; 10] = [
            0xffffff, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0xe0e0e0,
            0x000000, 0xffffff,
        ];
        match self {
            #[cfg(feature = "lvgl-tft")]
            Self::Tft => TFT[usize::from(index)],
            #[cfg(feature = "lvgl-mip")]
            Self::Mip => MIP[usize::from(index)],
        }
    }
}

unsafe extern "C" {
    fn kb_lvgl_begin(pixels: *mut u16, width: u16, top: u16, rows: u16, rgb: u32);
    fn kb_lvgl_rect(x: u16, y: u16, width: u16, height: u16, rgb: u32, radius: u8, outline: u8);
    fn kb_lvgl_text(x: u16, y: u16, rgb: u32, size: u8, text: *const c_char);
}

static BUSY: AtomicBool = AtomicBool::new(false);

struct DrawLock;

impl DrawLock {
    fn acquire() -> Self {
        while BUSY
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        Self
    }
}

impl Drop for DrawLock {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::Release);
    }
}

#[repr(C, align(4))]
pub struct Strip<const PIXELS: usize> {
    pixels: [u16; PIXELS],
}

impl<const PIXELS: usize> Strip<PIXELS> {
    pub const EMPTY: Self = Self {
        pixels: [0; PIXELS],
    };

    pub fn render(
        &mut self,
        profile: Profile,
        scene: &Scene,
        rows: Range<usize>,
    ) -> Option<&mut [u16]> {
        let (width, height) = profile.dimensions();
        if rows.start >= rows.end || rows.end > height || rows.len() * width > PIXELS {
            return None;
        }
        let pixels = &mut self.pixels[..rows.len() * width];
        let _lock = DrawLock::acquire();
        unsafe {
            kb_lvgl_begin(
                pixels.as_mut_ptr(),
                width as u16,
                rows.start as u16,
                rows.len() as u16,
                profile.color(0),
            );
        }
        for command in scene.commands() {
            draw(profile, command);
        }
        if profile.monochrome() {
            for pixel in pixels.iter_mut() {
                let red = *pixel >> 11;
                let green = (*pixel >> 5) & 63;
                let blue = *pixel & 31;
                *pixel = if red + green + blue >= 64 {
                    u16::MAX
                } else {
                    0
                };
            }
        }
        Some(pixels)
    }
}

#[cfg(feature = "lvgl-tft")]
const FONT_HEIGHTS: [usize; 4] = [30, 40, 52, 30];

#[cfg(feature = "lvgl-tft")]
fn intersects(command: &Command<'_>, rows: &Range<usize>) -> bool {
    let (y, height) = match *command {
        Command::Rect { y, height, .. } | Command::DotGrid { y, height, .. } => {
            (y, usize::from(height))
        }
        Command::Text { y, size, .. } => (y, FONT_HEIGHTS[usize::from(size)]),
    };
    let top = usize::from(y);
    top < rows.end && top + height > rows.start
}

#[cfg(feature = "lvgl-tft")]
pub fn same_band(before: &Scene, after: &Scene, rows: Range<usize>) -> bool {
    let commands = |command: &Command<'_>| intersects(command, &rows);
    before
        .commands()
        .filter(commands)
        .eq(after.commands().filter(commands))
}

fn draw(profile: Profile, command: Command<'_>) {
    match command {
        Command::Rect {
            x,
            y,
            width,
            height,
            color,
            radius,
            outline,
        } => {
            rect(x, y, width, height, profile.color(color), radius, outline);
        }
        Command::Text {
            x,
            y,
            color,
            size,
            value,
        } => {
            let mut text = [0; 256];
            text[..value.len()].copy_from_slice(value.as_bytes());
            let fonts = match profile {
                #[cfg(feature = "lvgl-tft")]
                Profile::Tft => [3, 4, 5, 3],
                #[cfg(feature = "lvgl-mip")]
                Profile::Mip => [1, 1, 2, 1],
            };
            unsafe {
                kb_lvgl_text(
                    x,
                    y,
                    profile.color(color),
                    fonts[usize::from(size)],
                    text.as_ptr().cast(),
                );
            }
        }
        Command::DotGrid {
            x,
            y,
            width,
            height,
            columns,
            rows,
            mask,
            color,
            underline,
            extra_gap,
        } => draw_grid(
            profile,
            (x, y),
            (width, height),
            (columns.into(), rows.into()),
            mask,
            color,
            (underline, extra_gap),
        ),
    }
}

fn rect(x: u16, y: u16, width: u16, height: u16, rgb: u32, radius: u8, outline: bool) {
    if width != 0 && height != 0 {
        unsafe {
            kb_lvgl_rect(x, y, width, height, rgb, radius, u8::from(outline));
        }
    }
}

fn draw_grid(
    profile: Profile,
    (x, y): (u16, u16),
    (width, height): (u16, u16),
    (columns, rows): (u16, u16),
    mask: u8,
    color: u8,
    (underline, extra_gap): (bool, bool),
) {
    let grid_height = height.saturating_sub(if underline { 4 } else { 0 });
    let size = (width / columns).min(grid_height / rows);
    if size == 0 {
        return;
    }
    let foreground = profile.color(color);
    let inactive = if profile.monochrome() { 0 } else { 0x526a7d };
    let gap = u16::from(size > 3) + u16::from(extra_gap);
    let diameter = size.saturating_sub(gap).max(1);
    let radius = (diameter / 2).min(u16::from(u8::MAX)) as u8;
    for index in 0..columns * rows {
        let col = u32::from(index % columns);
        let row = u32::from(index / columns);
        let offset_x =
            ((2 * col + 1) * u32::from(width) / (2 * u32::from(columns))) as u16 - diameter / 2;
        let offset_y =
            ((2 * row + 1) * u32::from(grid_height) / (2 * u32::from(rows))) as u16 - diameter / 2;
        let on = mask & (1 << index) != 0;
        let (diameter, inset) = if profile.monochrome() && !on && diameter <= 3 {
            (1, diameter / 2)
        } else {
            (diameter, 0)
        };
        rect(
            x.saturating_add(offset_x + inset),
            y.saturating_add(offset_y + inset),
            diameter,
            diameter,
            if on { foreground } else { inactive },
            radius,
            !on && diameter > 1,
        );
    }
    if underline {
        rect(
            x,
            y.saturating_add(height - 2),
            width,
            2,
            foreground,
            0,
            false,
        );
    }
}

#[cfg(all(target_arch = "wasm32", feature = "lvgl"))]
pub mod browser {
    use super::{Profile, Strip, TFT_WIDTH};
    use crate::{Scene, scene::CAPACITY};

    static mut INPUT: [u8; CAPACITY] = [0; CAPACITY];
    static mut STRIP: Strip<{ TFT_WIDTH * 8 }> = Strip::EMPTY;

    pub fn scene_buffer() -> *mut u8 {
        (&raw mut INPUT).cast()
    }

    pub unsafe fn render_scene(profile: u32, len: u32, top: u32, rows: u32) -> *const u16 {
        let render = || {
            let profile = match profile {
                0 => Profile::Tft,
                1 => Profile::Mip,
                _ => return None,
            };
            let bytes = unsafe { &*(&raw const INPUT) };
            let scene = Scene::from_bytes(bytes.get(..len as usize)?)?;
            if rows > 8 {
                return None;
            }
            let end = top.checked_add(rows)?;
            let strip = unsafe { &mut *(&raw mut STRIP) };
            Some(
                strip
                    .render(profile, &scene, top as usize..end as usize)?
                    .as_ptr(),
            )
        };
        render().unwrap_or(core::ptr::null())
    }
}
