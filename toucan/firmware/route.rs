use crate::usb_hid::Output;
use core::{
    cell::Cell,
    sync::atomic::{AtomicBool, Ordering},
};
use embassy_sync::{
    blocking_mutex::{Mutex, raw::CriticalSectionRawMutex},
    channel::Channel,
};
use embassy_time::{Duration, Timer, with_timeout};
use kb_core::{
    preferences::{ENCODED_LEN, Settings},
    transport::SessionGate,
};

pub(super) static OUTPUT: Channel<CriticalSectionRawMutex, (u32, Output), 64> = Channel::new();
static DIRECT: AtomicBool = AtomicBool::new(false);
static SESSION: Mutex<CriticalSectionRawMutex, Cell<SessionGate>> =
    Mutex::new(Cell::new(SessionGate::new()));

pub(super) fn direct() -> bool {
    DIRECT.load(Ordering::Relaxed)
}

pub(super) fn initialize() {
    DIRECT.store(settings().direct_bluetooth, Ordering::Relaxed);
}

pub(super) fn settings() -> Settings {
    crate::settings::current()
        .and_then(|blob| Settings::decode(blob.as_slice()).ok())
        .unwrap_or_default()
}

static IMPORTED: AtomicBool = AtomicBool::new(false);

pub(super) fn import(mut previous: Settings) {
    if crate::settings::current().is_none() {
        previous.direct_bluetooth = direct();
        if save(&previous) {
            IMPORTED.store(true, Ordering::Relaxed);
        }
    }
}

pub(super) fn take_import() -> Option<Settings> {
    IMPORTED.swap(false, Ordering::Relaxed).then(settings)
}

pub(super) fn switch(settings: &mut Settings) -> bool {
    settings.direct_bluetooth = !direct();
    DIRECT.store(settings.direct_bluetooth, Ordering::Relaxed);
    set_ready(false);
    save(settings)
}

pub(super) fn save(settings: &Settings) -> bool {
    let mut bytes = [0; ENCODED_LEN];
    settings.encode(&mut bytes).is_ok() && crate::settings::request_save(&bytes)
}

pub(super) async fn changed(previous: bool) {
    let epoch = crate::settings::radio_epoch();
    while direct() == previous && epoch == crate::settings::radio_epoch() {
        Timer::after_millis(10).await;
    }
}

pub(super) fn set_ready(ready: bool) {
    let gate = connection();
    if gate.ready() != ready {
        OUTPUT.clear();
        SESSION.lock(|state| state.set(gate.set_ready(ready)));
    }
}

pub(super) fn connection() -> SessionGate {
    SESSION.lock(Cell::get)
}

pub(super) fn invalidate_pending_reports() -> u32 {
    OUTPUT.clear();
    let next = SESSION.lock(|state| {
        let next = state.get().invalidate();
        state.set(next);
        next
    });
    if next.ready() {
        let _ = OUTPUT.try_send((
            next.generation(),
            Output::Stroke(kb_core::KeyStroke::default().held()),
        ));
    }
    next.generation()
}

pub(super) async fn send(generation: u32, output: Output) -> bool {
    if !connection().accepts(generation) {
        return false;
    }
    if with_timeout(
        Duration::from_millis(250),
        OUTPUT.send((generation, output)),
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
