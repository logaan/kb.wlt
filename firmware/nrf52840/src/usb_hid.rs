use core::cell::Cell;
use core::sync::atomic::{AtomicU8, AtomicU32, Ordering};
use embassy_futures::select::{Either, select};
use embassy_sync::blocking_mutex::{Mutex, raw::CriticalSectionRawMutex};
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;
use embassy_time::Timer;
use embassy_usb::Handler;
use embassy_usb::class::hid::{HidProtocolMode, HidWriter, ReportId, RequestHandler};
use embassy_usb::control::OutResponse;
use embassy_usb::driver::Driver;
use kb_core::KeyStroke;
use kb_core::transport::SessionGate;

static CONNECTION: Mutex<CriticalSectionRawMutex, Cell<SessionGate>> =
    Mutex::new(Cell::new(SessionGate::new()));
static CONNECTION_CHANGED: Signal<CriticalSectionRawMutex, ()> = Signal::new();

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
        let changed = CONNECTION.lock(|state| {
            let previous = state.get();
            let next = previous.set_ready(self.configured && !self.suspended);
            state.set(next);
            previous != next
        });
        if !self.configured || self.suspended {
            report_sent([0; 8]);
        }
        if changed {
            CONNECTION_CHANGED.signal(());
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
        CONNECTION_CHANGED.signal(());
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

fn report_sent(report: [u8; 8]) {
    CURRENT_REPORT.lock(|current| current.set(report));
}

async fn idle_timeout() {
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

async fn wait_for_session() -> u32 {
    loop {
        if let Some(token) = connection().token() {
            return token;
        }
        CONNECTION_CHANGED.wait().await;
    }
}

async fn session_ended(token: u32) {
    while connection().accepts(token) {
        CONNECTION_CHANGED.wait().await;
    }
}

async fn write_report<'d>(
    writer: &mut HidWriter<'d, impl Driver<'d>, 8>,
    token: u32,
    report: [u8; 8],
) -> bool {
    let write = async {
        writer.ready().await;
        writer.write(&report).await
    };
    if matches!(
        select(session_ended(token), write).await,
        Either::Second(Ok(()))
    ) {
        report_sent(report);
        true
    } else {
        false
    }
}

pub async fn send_strokes<'d, const N: usize>(
    writer: &mut HidWriter<'d, impl Driver<'d>, 8>,
    strokes: &Channel<CriticalSectionRawMutex, (u32, KeyStroke), N>,
) {
    send_reports(writer, strokes, KeyStroke::report, true).await;
}

pub async fn send_held_reports<'d, const N: usize>(
    writer: &mut HidWriter<'d, impl Driver<'d>, 8>,
    reports: &Channel<CriticalSectionRawMutex, (u32, [u8; 8]), N>,
) {
    send_reports(writer, reports, core::convert::identity, false).await;
}

async fn send_reports<'d, T, const N: usize>(
    writer: &mut HidWriter<'d, impl Driver<'d>, 8>,
    reports: &Channel<CriticalSectionRawMutex, (u32, T), N>,
    to_report: impl Fn(T) -> [u8; 8],
    release_each: bool,
) {
    loop {
        let token = wait_for_session().await;
        let mut previous = [0; 8];
        if !write_report(writer, token, previous).await {
            continue;
        }
        while let Either::Second(next) = select(
            session_ended(token),
            select(reports.receive(), idle_timeout()),
        )
        .await
        {
            let report = match next {
                Either::First((generation, value)) if generation == token => to_report(value),
                Either::First(_) => continue,
                Either::Second(()) => previous,
            };
            if !write_report(writer, token, report).await {
                break;
            }
            if release_each && report != [0; 8] && !write_report(writer, token, [0; 8]).await {
                break;
            }
            previous = if release_each { [0; 8] } else { report };
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
