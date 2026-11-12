use core::cell::Cell;
use core::sync::atomic::{AtomicU8, AtomicU32, Ordering};
use embassy_futures::select::{Either, select};
use embassy_sync::blocking_mutex::{Mutex, raw::CriticalSectionRawMutex};
use embassy_sync::signal::Signal;
use embassy_time::Timer;
use embassy_usb::Handler;
use embassy_usb::class::hid::{HidProtocolMode, ReportId, RequestHandler};
use embassy_usb::control::OutResponse;
use kb_core::transport::SessionGate;

static CONNECTION: Mutex<CriticalSectionRawMutex, Cell<SessionGate>> =
    Mutex::new(Cell::new(SessionGate::new()));

static CURRENT_REPORT: Mutex<CriticalSectionRawMutex, Cell<[u8; 8]>> =
    Mutex::new(Cell::new([0; 8]));
static PROTOCOL: AtomicU8 = AtomicU8::new(1);
static OUTPUT_LEDS: AtomicU8 = AtomicU8::new(0);
static IDLE_MS: AtomicU32 = AtomicU32::new(u32::MAX);
static IDLE_CHANGED: Signal<CriticalSectionRawMutex, ()> = Signal::new();

#[derive(Default)]
pub struct ConnectionState {
    configured: bool,
    suspended: bool,
}

pub fn connection() -> SessionGate {
    CONNECTION.lock(Cell::get)
}

impl ConnectionState {
    fn update(&self) {
        CONNECTION
            .lock(|state| state.set(state.get().set_ready(self.configured && !self.suspended)));
        if !self.configured || self.suspended {
            report_sent([0; 8]);
        }
    }
}

impl Handler for ConnectionState {
    fn configured(&mut self, configured: bool) {
        self.configured = configured;
        self.update();
    }

    fn reset(&mut self) {
        self.configured = false;
        self.suspended = false;
        CONNECTION.lock(|state| state.set(state.get().reset()));
        report_sent([0; 8]);
        PROTOCOL.store(1, Ordering::Relaxed);
        OUTPUT_LEDS.store(0, Ordering::Relaxed);
        IDLE_MS.store(u32::MAX, Ordering::Relaxed);
        IDLE_CHANGED.signal(());
    }

    fn enabled(&mut self, enabled: bool) {
        if !enabled {
            self.reset();
        }
    }

    fn suspended(&mut self, suspended: bool) {
        self.suspended = suspended;
        self.update();
    }
}

#[derive(Default)]
pub struct KeyboardControl;

pub fn report_sent(report: [u8; 8]) {
    CURRENT_REPORT.lock(|current| current.set(report));
}

pub async fn idle_timeout() {
    loop {
        if let Either::First(()) = select(
            Timer::after_millis(u64::from(IDLE_MS.load(Ordering::Relaxed))),
            IDLE_CHANGED.wait(),
        )
        .await
        {
            return;
        }
    }
}

impl RequestHandler for KeyboardControl {
    fn get_protocol(&self) -> HidProtocolMode {
        if PROTOCOL.load(Ordering::Relaxed) == 0 {
            HidProtocolMode::Boot
        } else {
            HidProtocolMode::Report
        }
    }

    fn set_protocol(&mut self, protocol: HidProtocolMode) -> OutResponse {
        PROTOCOL.store(protocol as u8, Ordering::Relaxed);
        OutResponse::Accepted
    }

    fn set_report(&mut self, id: ReportId, data: &[u8]) -> OutResponse {
        if id != ReportId::Out(0) || data.len() != 1 {
            return OutResponse::Rejected;
        }
        OUTPUT_LEDS.store(data[0], Ordering::Relaxed);
        OutResponse::Accepted
    }

    fn get_report(&mut self, id: ReportId, buffer: &mut [u8]) -> Option<usize> {
        match id {
            ReportId::In(0) if buffer.len() >= 8 => {
                CURRENT_REPORT.lock(|current| buffer[..8].copy_from_slice(&current.get()));
                Some(8)
            }
            ReportId::Out(0) if !buffer.is_empty() => {
                buffer[0] = OUTPUT_LEDS.load(Ordering::Relaxed);
                Some(1)
            }
            _ => None,
        }
    }

    fn get_idle_ms(&mut self, id: Option<ReportId>) -> Option<u32> {
        if id.is_none() || id == Some(ReportId::In(0)) {
            Some(IDLE_MS.load(Ordering::Relaxed))
        } else {
            None
        }
    }

    fn set_idle_ms(&mut self, id: Option<ReportId>, duration_ms: u32) {
        if id.is_none() || id == Some(ReportId::In(0)) {
            IDLE_MS.store(duration_ms.max(4), Ordering::Relaxed);
            IDLE_CHANGED.signal(());
        }
    }
}
