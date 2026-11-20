#![cfg(target_arch = "wasm32")]

use std::cell::RefCell;

use kb_client_session::{
    BOOTLOADER_PROSPECTOR, Change, Guest, Links, Model, Navigation, Session, Viewport,
};
use kb_core::KeyStroke;
use kb_core::braille_hid::{Report, stroke_reports};
use kb_core::host_target::{Target, TargetConnection};
use kb_core::matrix_link::{
    BRAILLE_THUMBS, BrailleSwitch, braille_keys, braille_transition_blocked,
};
use kb_core::pipeline::{Action, Key};
use kb_core::preferences::{ENCODED_LEN, Settings};
use kb_core::report_state::ReportState;

#[link(wasm_import_module = "host")]
unsafe extern "C" {
    fn guest_step(held: u16, chord: u16, keys: u16) -> u64;
    fn guest_map_cell(cell: u16) -> u32;
    fn guest_navigate(
        screen: u8,
        selected: u16,
        cell: u16,
        width: u16,
        height: u16,
        host_switch: bool,
    ) -> u32;
    fn guest_navigate_action(
        screen: u8,
        selected: u16,
        action: u8,
        width: u16,
        height: u16,
        host_switch: bool,
        receiver: bool,
    ) -> u32;
    fn keyboard_report(report: u64);
    fn consumer_report(report: u16);
    fn request_bootloader(devices: u8, receiver: bool) -> u8;
    fn request_target(target: u8);
}

enum BrowserGuest {
    Toucan,
    Prospector,
}

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
        view: Viewport,
    ) -> Result<u32, Self::Error> {
        Ok(unsafe {
            guest_navigate(
                screen,
                selected,
                cell,
                view.width,
                view.height,
                view.host_switch,
            )
        })
    }

    fn navigate_action(
        &mut self,
        screen: u8,
        selected: u16,
        action: Navigation,
        view: Viewport,
    ) -> Result<u32, Self::Error> {
        Ok(unsafe {
            guest_navigate_action(
                screen,
                selected,
                action as u8,
                view.width,
                view.height,
                view.host_switch,
                matches!(self, Self::Prospector),
            )
        })
    }
}

struct Emulator {
    layer_switch: BrailleSwitch,
    braille: bool,
    keys: u64,
    blocked: u64,
    session: Session,
    receiver: Session,
    receiver_linked: bool,
    touch_menu: bool,
    reports: ReportState<42>,
    consumer: [u8; 2],
    settings: [u8; ENCODED_LEN],
    model: Model,
}

impl Emulator {
    fn new() -> Self {
        let mut receiver = Session::new(Settings::default());
        receiver.enable_standalone_receiver_menu();
        receiver.set_bootloader_devices(BOOTLOADER_PROSPECTOR);
        Self {
            layer_switch: BrailleSwitch::default(),
            braille: false,
            keys: 0,
            blocked: 0,
            session: toucan_session(Settings::default()),
            receiver,
            receiver_linked: false,
            touch_menu: false,
            reports: ReportState::new(),
            consumer: [0; 2],
            settings: [0; ENCODED_LEN],
            model: Model::home(Links {
                online: 7,
                battery: [100; 2],
                target: Target::Prospector,
                connection: TargetConnection::Connected,
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
        let change = self
            .session
            .update(keys, now, &mut BrowserGuest::Toucan)
            .unwrap();
        if self.touch_menu && !self.session.menu_open() {
            self.blocked |= self.keys & !BRAILLE_THUMBS;
            self.touch_menu = false;
        }
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
        if change.bootloader != 0 {
            let accepted = unsafe { request_bootloader(change.bootloader, false) };
            self.session.bootloader_result(change.bootloader, accepted);
        }
        if let Some(target) = change.target {
            self.clear_reports();
            unsafe { request_target(target as u8) };
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
extern "C" fn braille_thumbs() -> u64 {
    BRAILLE_THUMBS
}

#[unsafe(no_mangle)]
extern "C" fn braille_mask(keys: u64) -> u16 {
    braille_keys(keys)
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
            if braille && !state.touch_menu {
                if !state.session.menu_open() {
                    state.session.cancel(0);
                }
            } else if !braille && !state.touch_menu {
                let final_keys = ready.then(|| braille_keys(layer_keys));
                let change = state
                    .session
                    .finish_momentary(final_keys, now, &mut BrowserGuest::Toucan)
                    .unwrap();
                saved = state.apply_braille_change(change);
                state.touch_menu = false;
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
    EMULATOR.with_borrow(|state| state.braille || state.touch_menu)
}

#[unsafe(no_mangle)]
extern "C" fn braille_update(keys: u16, now: u32) -> u32 {
    EMULATOR.with_borrow_mut(|state| state.update_braille(keys, now))
}

#[unsafe(no_mangle)]
extern "C" fn set_bootloader_devices(devices: u8) {
    EMULATOR.with_borrow_mut(|state| state.session.set_bootloader_devices(devices));
}

#[unsafe(no_mangle)]
extern "C" fn receiver_link_changed(linked: bool) {
    EMULATOR.with_borrow_mut(|state| {
        if state.receiver_linked != linked {
            state.receiver_linked = linked;
            state.receiver.cancel(0);
            if state.touch_menu {
                state.session.cancel(0);
                state.blocked |= state.keys & !BRAILLE_THUMBS;
                state.clear_reports();
                state.touch_menu = false;
            }
        }
    });
}

#[unsafe(no_mangle)]
extern "C" fn receiver_navigate(action: u8) -> u32 {
    let Some(action) = Navigation::from_byte(action) else {
        return 0;
    };
    EMULATOR.with_borrow_mut(|state| {
        if state.receiver_linked {
            let opened = state.session.menu_open();
            let was_touch_menu = state.touch_menu;
            let change = state
                .session
                .navigate_action(action, &mut BrowserGuest::Toucan)
                .unwrap();
            state.touch_menu = state.session.menu_open() && (!opened || state.touch_menu);
            let reset = change.reset || (was_touch_menu && !state.touch_menu);
            if reset {
                state.blocked |= state.keys & !BRAILLE_THUMBS;
            }
            (u32::from(reset) << 1) | state.apply_braille_change(change)
        } else {
            let change = state
                .receiver
                .navigate_action(action, &mut BrowserGuest::Prospector)
                .unwrap();
            if change.bootloader == BOOTLOADER_PROSPECTOR {
                let accepted = unsafe { request_bootloader(change.bootloader, true) };
                state
                    .receiver
                    .bootloader_result(change.bootloader, accepted);
            }
            0
        }
    })
}

#[unsafe(no_mangle)]
extern "C" fn restart_receiver() {
    EMULATOR.with_borrow_mut(|state| state.receiver.cancel(0));
}

#[unsafe(no_mangle)]
extern "C" fn restart_keyboard() {
    EMULATOR.with_borrow_mut(|state| {
        state.layer_switch = BrailleSwitch::default();
        state.braille = false;
        state.session.cancel(0);
        state.touch_menu = false;
        state.clear_reports();
    });
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
extern "C" fn set_host_target(target: u8) {
    if let Some(target) = Target::from_byte(target) {
        EMULATOR.with_borrow_mut(|state| state.session.set_host_target(target));
    }
}

#[unsafe(no_mangle)]
extern "C" fn host_connection_changed() {
    EMULATOR.with_borrow_mut(|state| {
        state.blocked |= state.keys & !BRAILLE_THUMBS;
        state.session.cancel_for_link_change(0);
        state.clear_reports();
    });
}

#[unsafe(no_mangle)]
extern "C" fn update_model(
    online: u8,
    left: u8,
    right: u8,
    target: u8,
    connection: u8,
    standalone: bool,
) {
    let links = Links {
        online,
        battery: [left, right],
        target: Target::from_byte(target).unwrap_or(Target::Unknown),
        connection: TargetConnection::from_byte(connection),
    };
    EMULATOR.with_borrow_mut(|state| {
        state.model = if standalone && state.receiver.menu_open() {
            state.receiver.model(links)
        } else if !standalone && (state.braille || state.session.menu_open()) {
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
