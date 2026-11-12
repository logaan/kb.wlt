use core::cell::Cell;
use core::sync::atomic::{AtomicU8, AtomicU32, Ordering};
use embassy_futures::select::{Either, select};
use embassy_sync::blocking_mutex::{Mutex, raw::CriticalSectionRawMutex};
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Timer, with_timeout};
use embassy_usb::Handler;
use embassy_usb::class::hid::{
    Config, HidBootProtocol, HidProtocolMode, HidSubclass, HidWriter, ReportId, RequestHandler,
    State,
};
use embassy_usb::control::OutResponse;
use embassy_usb::driver::Driver;
use kb_core::KeyStroke;
use kb_core::braille_hid::{Report, stroke_reports};
use kb_core::transport::SessionGate;

static CONNECTION: Mutex<CriticalSectionRawMutex, Cell<SessionGate>> =
    Mutex::new(Cell::new(SessionGate::new()));
static CONNECTION_CHANGED: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static BUS_RESET: AtomicU32 = AtomicU32::new(0);

static CURRENT_REPORT: Mutex<CriticalSectionRawMutex, Cell<[u8; 8]>> =
    Mutex::new(Cell::new([0; 8]));
static CURRENT_CONSUMER: Mutex<CriticalSectionRawMutex, Cell<[u8; 2]>> =
    Mutex::new(Cell::new([0; 2]));
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

pub fn invalidate_pending_reports() -> u32 {
    let next = CONNECTION.lock(|state| {
        let next = state.get().invalidate();
        state.set(next);
        next
    });
    report_sent([0; 8]);
    CURRENT_CONSUMER.lock(|current| current.set([0; 2]));
    CONNECTION_CHANGED.signal(());
    next.generation()
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
            CURRENT_CONSUMER.lock(|current| current.set([0; 2]));
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
        CURRENT_CONSUMER.lock(|current| current.set([0; 2]));
        BUS_RESET.fetch_add(1, Ordering::Relaxed);
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

pub struct ConsumerControl;

impl RequestHandler for ConsumerControl {
    fn get_report(&mut self, id: ReportId, buffer: &mut [u8]) -> Option<usize> {
        if id != ReportId::In(0) || buffer.len() < 2 {
            return None;
        }
        CURRENT_CONSUMER.lock(|current| buffer[..2].copy_from_slice(&current.get()));
        Some(2)
    }
}

pub fn consumer_writer<'d, D: Driver<'d>>(
    builder: &mut embassy_usb::Builder<'d, D>,
    state: &'d mut State<'d>,
    control: &'d mut ConsumerControl,
) -> HidWriter<'d, D, 2> {
    HidWriter::new(
        builder,
        state,
        Config {
            report_descriptor: &kb_core::hid::CONSUMER_REPORT_DESCRIPTOR,
            request_handler: Some(control),
            poll_ms: 1,
            max_packet_size: 2,
            hid_subclass: HidSubclass::No,
            hid_boot_protocol: HidBootProtocol::None,
        },
    )
}

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
    consumer: &mut HidWriter<'d, impl Driver<'d>, 2>,
    strokes: &Channel<CriticalSectionRawMutex, (u32, KeyStroke), N>,
) {
    let mut bus_reset = BUS_RESET.load(Ordering::Relaxed);
    let mut previous_consumer = [0; 2];
    let mut ordered_release_pending = false;
    loop {
        let token = wait_for_session().await;
        let reset = BUS_RESET.load(Ordering::Relaxed);
        if reset != bus_reset {
            previous_consumer = [0; 2];
            ordered_release_pending = false;
            bus_reset = reset;
        }
        let mut keyboard = [0; 8];
        let mut consumer_confirmed = false;
        let was_active = previous_consumer != [0; 2] || ordered_release_pending;
        if !write_report(writer, token, keyboard).await
            || (was_active && !write_report(writer, token, keyboard).await)
        {
            continue;
        }
        let mut release_pending = !release_consumer(consumer, token, &mut previous_consumer).await;
        ordered_release_pending = was_active && release_pending;
        'session: while let Either::Second(next) = select(session_ended(token), async {
            if release_pending {
                select(strokes.receive(), Timer::after_millis(10)).await
            } else {
                select(strokes.receive(), idle_timeout()).await
            }
        })
        .await
        {
            let stroke = match next {
                Either::First((generation, stroke)) if generation == token => stroke,
                Either::First(_) => continue,
                Either::Second(()) if release_pending => {
                    if release_consumer(consumer, token, &mut previous_consumer).await {
                        consumer_confirmed = false;
                        release_pending = false;
                        ordered_release_pending = false;
                    }
                    continue;
                }
                Either::Second(()) => {
                    if !write_report(writer, token, keyboard).await {
                        break;
                    }
                    continue;
                }
            };
            let requested_consumer = stroke.consumer_report();
            if ordered_release_pending && requested_consumer == [0; 2] && stroke.report() != [0; 8]
            {
                if !release_consumer(consumer, token, &mut previous_consumer).await {
                    continue;
                }
                ordered_release_pending = false;
                release_pending = false;
                consumer_confirmed = false;
            }
            let mut reports = stroke_reports(previous_consumer, stroke)
                .into_iter()
                .flatten()
                .peekable();
            while let Some(report) = reports.next() {
                match report {
                    Report::Keyboard(report) => {
                        if report != [0; 8] && requested_consumer != [0; 2] {
                            if previous_consumer != requested_consumer {
                                continue;
                            }
                            if !consumer_confirmed {
                                consumer_confirmed =
                                    write_consumer(consumer, token, requested_consumer).await;
                            }
                            if !consumer_confirmed {
                                continue;
                            }
                        }
                        if !write_report(writer, token, report).await {
                            break 'session;
                        }
                        keyboard = report;
                        if matches!(reports.peek(), Some(Report::Consumer(_)))
                            && !write_report(writer, token, report).await
                        {
                            break 'session;
                        }
                    }
                    Report::Consumer([0, 0]) => {
                        let was_active = previous_consumer != [0; 2] || ordered_release_pending;
                        release_pending =
                            !release_consumer(consumer, token, &mut previous_consumer).await;
                        ordered_release_pending = was_active && release_pending;
                        consumer_confirmed = false;
                    }
                    Report::Consumer(report) => {
                        if write_consumer(consumer, token, report).await {
                            previous_consumer = report;
                            consumer_confirmed = false;
                            release_pending = false;
                            ordered_release_pending = false;
                        }
                    }
                }
            }
        }
    }
}

async fn release_consumer<'d>(
    writer: &mut HidWriter<'d, impl Driver<'d>, 2>,
    token: u32,
    previous: &mut [u8; 2],
) -> bool {
    if !write_consumer(writer, token, [0; 2]).await {
        return false;
    }
    let previous = core::mem::replace(previous, [0; 2]);
    previous == [0; 2] || write_consumer(writer, token, [0; 2]).await
}

async fn write_consumer<'d>(
    writer: &mut HidWriter<'d, impl Driver<'d>, 2>,
    token: u32,
    report: [u8; 2],
) -> bool {
    let write = async {
        writer.ready().await;
        writer.write(&report).await
    };
    if matches!(
        select(
            session_ended(token),
            with_timeout(Duration::from_millis(8), write)
        )
        .await,
        Either::Second(Ok(Ok(())))
    ) {
        CURRENT_CONSUMER.lock(|current| current.set(report));
        true
    } else {
        false
    }
}

pub async fn send_held_reports<'d, const N: usize>(
    writer: &mut HidWriter<'d, impl Driver<'d>, 8>,
    reports: &Channel<CriticalSectionRawMutex, (u32, [u8; 8]), N>,
) {
    send_reports(writer, reports).await;
}

async fn send_reports<'d, const N: usize>(
    writer: &mut HidWriter<'d, impl Driver<'d>, 8>,
    reports: &Channel<CriticalSectionRawMutex, (u32, [u8; 8]), N>,
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
                Either::First((generation, value)) if generation == token => value,
                Either::First(_) => continue,
                Either::Second(()) => previous,
            };
            if !write_report(writer, token, report).await {
                break;
            }
            previous = report;
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
