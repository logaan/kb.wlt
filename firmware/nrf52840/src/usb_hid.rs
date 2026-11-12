use core::cell::Cell;
use core::sync::atomic::{AtomicU8, AtomicU32, Ordering};
use embassy_futures::select::{Either, select};
use embassy_sync::blocking_mutex::{Mutex, raw::CriticalSectionRawMutex};
use embassy_sync::signal::Signal;
use embassy_time::Timer;
use embassy_usb::class::hid::{HidProtocolMode, ReportId, RequestHandler};
use embassy_usb::control::OutResponse;

static CURRENT_REPORT: Mutex<CriticalSectionRawMutex, Cell<[u8; 8]>> =
    Mutex::new(Cell::new([0; 8]));
static OUTPUT_LEDS: AtomicU8 = AtomicU8::new(0);
static IDLE_MS: AtomicU32 = AtomicU32::new(u32::MAX);
static IDLE_CHANGED: Signal<CriticalSectionRawMutex, ()> = Signal::new();

pub struct KeyboardControl {
    protocol: HidProtocolMode,
}

impl Default for KeyboardControl {
    fn default() -> Self {
        Self {
            protocol: HidProtocolMode::Report,
        }
    }
}

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
        self.protocol
    }

    fn set_protocol(&mut self, protocol: HidProtocolMode) -> OutResponse {
        self.protocol = protocol;
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
