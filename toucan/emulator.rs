#![cfg(target_arch = "wasm32")]

use std::cell::RefCell;

use kb_client_session::{Change, Guest, Links, Model, Session};
use kb_core::KeyStroke;
use kb_core::braille_hid::{Report, stroke_reports};
use kb_core::matrix_link::{
    BRAILLE_THUMB, BrailleSwitch, RouteSwitch, braille_keys, braille_transition_blocked,
};
use kb_core::pipeline::{Action, Key};
use kb_core::preferences::{ENCODED_LEN, Settings};
use kb_core::report_state::ReportState;

#[link(wasm_import_module = "host")]
unsafe extern "C" {
    fn guest_step(held: u16, chord: u16, keys: u16) -> u64;
    fn guest_map_cell(cell: u16) -> u32;
    fn guest_navigate(screen: u8, selected: u16, cell: u16, visible: u8) -> u32;
    fn keyboard_report(report: u64);
    fn consumer_report(report: u16);
}

struct BrowserGuest;

impl Guest for BrowserGuest {
    type Error = core::convert::Infallible;

    fn step(
        &mut self,
        held: u16,
        chord: u16,
        keys: u16,
    ) -> Result<(u16, u16, Option<KeyStroke>), Self::Error> {
        let output = unsafe { guest_step(held, chord, keys) };
        Ok((
            output as u16,
            (output >> 16) as u16,
            KeyStroke::from_packed((output >> 32) as u32),
        ))
    }

    fn map_cell(&mut self, cell: u16) -> Result<u32, Self::Error> {
        Ok(unsafe { guest_map_cell(cell) })
    }

    fn navigate(
        &mut self,
        screen: u8,
        selected: u16,
        cell: u16,
        visible: u8,
    ) -> Result<u32, Self::Error> {
        Ok(unsafe { guest_navigate(screen, selected, cell, visible) })
    }
}

struct Emulator {
    layer_switch: BrailleSwitch,
    route_switch: RouteSwitch,
    braille: bool,
    keys: u64,
    blocked: u64,
    session: Session,
    reports: ReportState<42>,
    consumer: [u8; 2],
    settings: [u8; ENCODED_LEN],
    model: Model,
}

impl Emulator {
    fn new() -> Self {
        Self {
            layer_switch: BrailleSwitch::default(),
            route_switch: RouteSwitch::new(),
            braille: false,
            keys: 0,
            blocked: 0,
            session: toucan_session(Settings::default()),
            reports: ReportState::new(),
            consumer: [0; 2],
            settings: [0; ENCODED_LEN],
            model: Model::home(Links {
                online: 7,
                battery: [100; 2],
            }),
        }
    }

    fn clear_reports(&mut self) {
        self.reports.clear();
        unsafe { keyboard_report(0) };
        if self.consumer != [0; 2] {
            self.consumer = [0; 2];
            unsafe { consumer_report(0) };
        }
    }

    fn update_braille(&mut self, keys: u16, now: u32) -> u32 {
        let change = self.session.update(keys, now, &mut BrowserGuest).unwrap();
        self.apply_braille_change(change)
    }

    fn apply_braille_change(&mut self, change: Change) -> u32 {
        if change.reset {
            self.clear_reports();
        }
        for stroke in change.strokes.as_slice() {
            for report in stroke_reports(self.consumer, *stroke).into_iter().flatten() {
                match report {
                    Report::Keyboard(bytes) => unsafe {
                        keyboard_report(u64::from_le_bytes(bytes))
                    },
                    Report::Consumer(bytes) => unsafe {
                        consumer_report(u16::from_le_bytes(bytes))
                    },
                }
            }
            self.consumer = if stroke.is_held() {
                stroke.consumer_report()
            } else {
                [0; 2]
            };
        }
        u32::from(change.save && self.session.settings.encode(&mut self.settings).is_ok())
    }
}

fn toucan_session(settings: Settings) -> Session {
    let mut session = Session::new(settings);
    session.enable_host_switch();
    session
}

thread_local! {
    static EMULATOR: RefCell<Emulator> = RefCell::new(Emulator::new());
}

#[unsafe(no_mangle)]
extern "C" fn braille_thumb() -> u8 {
    BRAILLE_THUMB
}

#[unsafe(no_mangle)]
extern "C" fn braille_mask(keys: u64) -> u16 {
    braille_keys(keys)
}

#[unsafe(no_mangle)]
extern "C" fn route_update(keys: u64, now: u32) -> u8 {
    EMULATOR.with_borrow_mut(|state| {
        let was_blocked = state.route_switch.blocked();
        let changed = state.route_switch.update(now, keys);
        let blocked = state.route_switch.blocked();
        if blocked {
            state.blocked |= keys;
        }
        if blocked && !was_blocked {
            state.session.cancel(0);
            state.clear_reports();
        }
        if changed { 2 } else { u8::from(blocked) }
    })
}

#[unsafe(no_mangle)]
extern "C" fn matrix(keys: u64, now: u32, ready: bool) -> u64 {
    EMULATOR.with_borrow_mut(|state| {
        state.blocked &= keys;
        let layer_keys = keys & !state.blocked;
        let braille = state.layer_switch.update(now, ready.then_some(layer_keys));
        let mut saved = 0;
        if braille != state.braille {
            state.blocked |= braille_transition_blocked(braille, state.keys, keys);
            if braille {
                state.session.cancel(0);
            } else {
                let final_keys = ready.then(|| braille_keys(layer_keys));
                let change = state
                    .session
                    .finish_momentary(final_keys, now, &mut BrowserGuest)
                    .unwrap();
                saved = state.apply_braille_change(change);
            }
            state.clear_reports();
            state.braille = braille;
        }
        if !ready && state.keys != 0 {
            state.session.cancel(0);
            state.clear_reports();
        }
        state.keys = if ready { keys } else { 0 };
        (state.keys & !state.blocked) | (u64::from(saved) << 63)
    })
}

#[unsafe(no_mangle)]
extern "C" fn braille_active() -> bool {
    EMULATOR.with_borrow(|state| state.braille)
}

#[unsafe(no_mangle)]
extern "C" fn braille_update(keys: u16, now: u32) -> u32 {
    EMULATOR.with_borrow_mut(|state| state.update_braille(keys, now))
}

#[unsafe(no_mangle)]
extern "C" fn key_action(tag: u8, usage: u16, modifiers: u8) {
    let key = Key { usage, modifiers };
    let action = match tag {
        0 => Action::Press(key),
        1 => Action::Release(key),
        2 => Action::Tap(key),
        _ => Action::ReleaseAll,
    };
    EMULATOR.with_borrow_mut(|state| {
        let reports = state.reports.apply(action).unwrap();
        unsafe { keyboard_report(u64::from_le_bytes(reports.first)) };
        if let Some(report) = reports.second {
            unsafe { keyboard_report(u64::from_le_bytes(report)) };
        }
    });
}

#[unsafe(no_mangle)]
extern "C" fn set_viewport(width: u16, height: u16) {
    EMULATOR.with_borrow_mut(|state| state.session.set_viewport(width, height));
}

#[unsafe(no_mangle)]
extern "C" fn update_model(online: u8, left: u8, right: u8, direct: bool, home: bool) {
    let links = Links {
        online: online | (u8::from(direct) << 4),
        battery: [left, right],
    };
    EMULATOR.with_borrow_mut(|state| {
        state.model = if state.braille && !home {
            state.session.model(links)
        } else {
            Model::home(links)
        };
    });
}

#[unsafe(no_mangle)]
extern "C" fn model_bytes() -> *const u8 {
    EMULATOR.with_borrow(|state| state.model.bytes.as_slice().as_ptr())
}

#[unsafe(no_mangle)]
extern "C" fn model_len() -> usize {
    EMULATOR.with_borrow(|state| state.model.bytes.as_slice().len())
}

#[unsafe(no_mangle)]
extern "C" fn model_detail() -> *const u8 {
    EMULATOR.with_borrow(|state| state.model.detail.as_str().as_ptr())
}

#[unsafe(no_mangle)]
extern "C" fn model_detail_len() -> usize {
    EMULATOR.with_borrow(|state| state.model.detail.as_str().len())
}

#[unsafe(no_mangle)]
extern "C" fn settings_buffer() -> *mut u8 {
    EMULATOR.with_borrow_mut(|state| state.settings.as_mut_ptr())
}

#[unsafe(no_mangle)]
extern "C" fn settings_len() -> usize {
    ENCODED_LEN
}

#[unsafe(no_mangle)]
extern "C" fn load_settings() {
    EMULATOR.with_borrow_mut(|state| {
        if let Ok(settings) = Settings::decode(&state.settings) {
            state.session = toucan_session(settings);
        }
    });
}

#[unsafe(no_mangle)]
extern "C" fn lvgl_scene_buffer() -> *mut u8 {
    kb_display::lvgl::browser::scene_buffer()
}

#[unsafe(no_mangle)]
extern "C" fn lvgl_render(profile: u32, len: u32, top: u32, rows: u32) -> *const u16 {
    unsafe { kb_display::lvgl::browser::render_scene(profile, len, top, rows) }
}
