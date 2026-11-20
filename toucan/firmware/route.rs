use crate::usb_hid::Output;
use core::{
    cell::Cell,
    sync::atomic::{AtomicBool, AtomicU8, Ordering},
};
use embassy_sync::{
    blocking_mutex::{Mutex, raw::CriticalSectionRawMutex},
    channel::Channel,
    signal::Signal,
};
use embassy_time::{Duration, Timer, with_timeout};
use kb_core::{
    host_target::{
        TOUCAN_CONFIG_LEN, Target, TargetConnection, decode_toucan_config, encode_toucan_config,
    },
    preferences::Settings,
    transport::SessionGate,
};

type Reports = Channel<CriticalSectionRawMutex, (u32, Output), 64>;

pub(super) static PROSPECTOR_REPORTS: Reports = Channel::new();
pub(super) static BLUETOOTH_REPORTS: Reports = Channel::new();
pub(super) static USB_REPORTS: Reports = Channel::new();
static TARGET: AtomicU8 = AtomicU8::new(Target::Prospector as u8);
static SESSION: Mutex<CriticalSectionRawMutex, Cell<SessionGate>> =
    Mutex::new(Cell::new(SessionGate::new()));
static SESSION_CHANGED: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static RELEASED: Signal<CriticalSectionRawMutex, u32> = Signal::new();
static RESET_PROSPECTOR: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static IMPORTED: AtomicBool = AtomicBool::new(false);

pub(super) fn target() -> Target {
    Target::from_byte(TARGET.load(Ordering::Relaxed)).unwrap()
}

fn reports(target: Target) -> &'static Reports {
    match target {
        Target::Prospector => &PROSPECTOR_REPORTS,
        Target::Usb => &USB_REPORTS,
        _ => &BLUETOOTH_REPORTS,
    }
}

pub(super) fn initialize() {
    TARGET.store(config().0 as u8, Ordering::Relaxed);
}

pub(super) fn settings() -> Settings {
    config().1
}

fn config() -> (Target, Settings) {
    crate::settings::current()
        .map(|blob| decode_toucan_config(blob.as_slice()))
        .unwrap_or_default()
}

pub(super) fn import(previous: Settings) {
    if crate::settings::current().is_none() {
        if save(&previous) {
            IMPORTED.store(true, Ordering::Relaxed);
        }
    }
}

pub(super) fn take_import() -> Option<Settings> {
    IMPORTED.swap(false, Ordering::Relaxed).then(settings)
}

pub(super) async fn switch(selected: Target, settings: &mut Settings) -> bool {
    let previous = target();
    if previous == selected {
        return !crate::settings::unsaved() || save(settings);
    }
    if connection().ready() {
        let generation = invalidate();
        RELEASED.reset();
        let _ = reports(previous).try_send((generation, Output::Release(generation)));
        let released = with_timeout(Duration::from_millis(200), async {
            while RELEASED.wait().await != generation {}
        })
        .await;
        if released.is_err() && previous == Target::Prospector {
            RESET_PROSPECTOR.signal(());
        }
    }
    set_ready(false);
    if IMPORTED.load(Ordering::Relaxed) {
        *settings = self::settings();
    }
    TARGET.store(selected as u8, Ordering::Relaxed);
    settings.direct_bluetooth = selected.is_bluetooth();
    save(settings)
}

pub(super) fn save(settings: &Settings) -> bool {
    let mut bytes = [0; TOUCAN_CONFIG_LEN];
    encode_toucan_config(target(), settings, &mut bytes).is_ok()
        && crate::settings::request_save(&bytes)
}

pub(super) async fn changed(previous: Target) {
    while target() == previous {
        Timer::after_millis(10).await;
    }
}

fn set_ready(ready: bool) {
    let gate = connection();
    if gate.ready() != ready {
        reports(target()).clear();
        SESSION.lock(|state| state.set(gate.set_ready(ready)));
        SESSION_CHANGED.signal(());
    }
}

pub(super) fn set_transport_ready(transport: Target, ready: bool) {
    if target() == transport
        && (!ready || transport == Target::Usb || crate::settings::bluetooth_enabled())
    {
        set_ready(ready);
    }
}

pub(super) fn connection() -> SessionGate {
    SESSION.lock(Cell::get)
}

pub(super) fn status(docked: bool) -> TargetConnection {
    let selected = target();
    if connection().ready() {
        TargetConnection::Connected
    } else if selected == Target::Usb
        || !crate::settings::bluetooth_enabled()
        || (selected == Target::Prospector && docked)
    {
        TargetConnection::Offline
    } else {
        TargetConnection::Connecting
    }
}

pub(super) async fn session_ended(generation: u32) {
    while connection().accepts(generation) {
        SESSION_CHANGED.wait().await;
    }
}

fn invalidate() -> u32 {
    reports(target()).clear();
    let next = SESSION.lock(|state| {
        let next = state.get().invalidate();
        state.set(next);
        next
    });
    SESSION_CHANGED.signal(());
    next.generation()
}

pub(super) fn invalidate_pending_reports() -> u32 {
    let generation = invalidate();
    if connection().ready() {
        let _ = reports(target()).try_send((
            generation,
            Output::Stroke(kb_core::KeyStroke::default().held()),
        ));
    }
    generation
}

pub(super) async fn reset_prospector() {
    RESET_PROSPECTOR.wait().await;
}

pub(super) fn released(generation: u32) {
    RELEASED.signal(generation);
}

pub(super) async fn send(generation: u32, output: Output) -> bool {
    if !connection().accepts(generation) {
        return false;
    }
    if with_timeout(
        Duration::from_millis(250),
        reports(target()).send((generation, output)),
    )
    .await
    .is_err()
    {
        if connection().accepts(generation) {
            invalidate_pending_reports();
        }
        return false;
    }
    true
}
