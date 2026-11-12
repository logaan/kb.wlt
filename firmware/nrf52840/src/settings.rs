use core::cell::Cell;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use embassy_futures::select::{Either, select};
use embassy_sync::blocking_mutex::{Mutex, raw::CriticalSectionRawMutex};
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;
use embassy_time::Timer;
use kb_core::storage::SaveState;

#[derive(Clone, Copy)]
pub struct ConfigBlob {
    bytes: [u8; 512],
    length: u16,
    revision: u32,
}

impl ConfigBlob {
    fn new(payload: &[u8], revision: u32) -> Option<Self> {
        if payload.len() > 512 {
            return None;
        }
        let mut bytes = [0; 512];
        bytes[..payload.len()].copy_from_slice(payload);
        Some(Self {
            bytes,
            length: payload.len() as u16,
            revision,
        })
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.bytes[..usize::from(self.length)]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BluetoothControl {
    Disconnect,
    Reconnect,
    ClearBonds,
    PairNew,
}

static CURRENT: Mutex<CriticalSectionRawMutex, Cell<Option<ConfigBlob>>> =
    Mutex::new(Cell::new(None));
static SAVES: Signal<CriticalSectionRawMutex, ConfigBlob> = Signal::new();
static CONTROLS: Channel<CriticalSectionRawMutex, BluetoothControl, 4> = Channel::new();
static SAVE_STATE: Mutex<CriticalSectionRawMutex, Cell<SaveState>> =
    Mutex::new(Cell::new(SaveState::new()));
static RADIO_ENABLED: AtomicBool = AtomicBool::new(true);
static RADIO_EPOCH: AtomicU32 = AtomicU32::new(0);

pub fn current() -> Option<ConfigBlob> {
    CURRENT.lock(Cell::get)
}

pub(crate) fn initialize(payload: Option<&[u8]>) {
    CURRENT.lock(|current| current.set(payload.and_then(|payload| ConfigBlob::new(payload, 0))));
}

pub fn request_save(payload: &[u8]) -> bool {
    let Some(mut blob) = ConfigBlob::new(payload, 0) else {
        return false;
    };
    blob.revision = SAVE_STATE.lock(|state| {
        let (next, revision) = state.get().requested();
        state.set(next);
        revision
    });
    CURRENT.lock(|current| current.set(Some(blob)));
    SAVES.signal(blob);
    true
}

pub fn unsaved() -> bool {
    SAVE_STATE.lock(|state| state.get().unsaved())
}

pub fn request_bluetooth(control: BluetoothControl) -> bool {
    CONTROLS.try_send(control).is_ok()
}

pub fn bluetooth_enabled() -> bool {
    RADIO_ENABLED.load(Ordering::Relaxed)
}

pub(crate) fn radio_epoch() -> u32 {
    RADIO_EPOCH.load(Ordering::Relaxed)
}

pub(crate) async fn radio_changed(epoch: u32) {
    while radio_epoch() == epoch {
        Timer::after_millis(25).await;
    }
}

pub(crate) async fn run_saves(bonds: &crate::split::Bonds) {
    loop {
        let mut blob = SAVES.wait().await;
        let mut attempts = 0;
        loop {
            let failed = bonds
                .lock()
                .await
                .save_config(blob.as_slice())
                .await
                .is_err();
            SAVE_STATE.lock(|state| state.set(state.get().completed(blob.revision, !failed)));
            if !failed {
                break;
            }
            attempts += 1;
            if attempts >= 3 {
                break;
            }
            match select(SAVES.wait(), Timer::after_millis(250)).await {
                Either::First(newer) => {
                    blob = newer;
                    attempts = 0;
                }
                Either::Second(_) => {}
            }
        }
    }
}

pub(crate) async fn run_controls(stack: &crate::split::Host<'_>, bonds: &crate::split::Bonds) {
    loop {
        let control = CONTROLS.receive().await;
        RADIO_ENABLED.store(false, Ordering::Relaxed);
        RADIO_EPOCH.fetch_add(1, Ordering::Relaxed);
        Timer::after_millis(150).await;
        if matches!(
            control,
            BluetoothControl::ClearBonds | BluetoothControl::PairNew
        ) {
            while let Some(identity) =
                stack.with_bond_information(|bonds| bonds.first().map(|bond| bond.identity))
            {
                if stack.remove_bond_information(identity).is_err() {
                    break;
                }
            }
            bonds.lock().await.forget().await;
        }
        Timer::after_millis(if control == BluetoothControl::Disconnect {
            3000
        } else {
            150
        })
        .await;
        RADIO_ENABLED.store(true, Ordering::Relaxed);
        RADIO_EPOCH.fetch_add(1, Ordering::Relaxed);
    }
}
