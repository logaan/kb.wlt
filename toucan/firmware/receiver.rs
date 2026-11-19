use super::*;
#[cfg(feature = "toucan-dongle")]
use crate::braille_receiver::OUTPUT as REPORTS;
use crate::radio::Irqs;
use embassy_futures::select::Either;
use embassy_nrf::usb::{Driver, vbus_detect::HardwareVbusDetect};
use embassy_nrf::{Peri, peripherals};
use embassy_sync::channel::Channel;
use embassy_sync::mutex::Mutex;
use embassy_time::with_timeout;
use embassy_usb::class::hid::{
    Config as HidConfig, HidBootProtocol, HidSubclass, HidWriter, State,
};
use embassy_usb::{Builder, Config};
use kb_component_runtime::KeyboardComponent;
use kb_core::hid::KEYBOARD_REPORT_DESCRIPTOR as REPORT_DESCRIPTOR;
use kb_core::matrix_link::Receiver;
use kb_core::pipeline::Bounded;
use kb_core::pipeline::{Input, KeyEvent, KeyState};
use kb_core::report_state::ReportState;
use kb_core::transport::{InputGate, SessionGate};
static EVENTS: Channel<CriticalSectionRawMutex, LinkEvent, 64> = Channel::new();
static ADVERTISEMENTS: Channel<CriticalSectionRawMutex, Discovered, 16> = Channel::new();
static CONNECT_LOCK: Mutex<CriticalSectionRawMutex, ()> = Mutex::new(());

#[derive(Clone, Copy)]
struct Discovered {
    address: Address,
    side: Side,
    display_feedback: bool,
}

#[derive(Clone, Copy)]
enum LinkEvent {
    Snapshot {
        snapshot: Snapshot,
        received_at: u32,
        opens: bool,
    },
    Close(Side),
}

impl LinkEvent {
    fn snapshot(snapshot: Snapshot, opens: bool) -> Self {
        Self::Snapshot {
            snapshot,
            received_at: Instant::now().as_millis() as u32,
            opens,
        }
    }
}

pub(super) struct Discovery;

impl EventHandler for Discovery {
    fn on_adv_reports(&self, mut reports: LeAdvReportsIter<'_>) {
        while let Some(Ok(report)) = reports.next() {
            if let Some((side, display_feedback)) = advertised_peer(report.data) {
                let _ = ADVERTISEMENTS.try_send(Discovered {
                    address: Address::new(report.addr_kind, report.addr),
                    side,
                    display_feedback,
                });
            }
        }
    }
}

fn advertised_peer(mut data: &[u8]) -> Option<(Side, bool)> {
    let mut service = false;
    let mut side = None;
    while !data.is_empty() {
        let length = data[0] as usize;
        if length == 0 {
            break;
        }
        if length >= data.len() {
            return None;
        }
        let field = &data[1..=length];
        if field[0] == 0x07
            && field[1..]
                .chunks_exact(16)
                .any(|uuid| uuid == SERVICE_BYTES)
        {
            service = true;
        }
        if field.len() == 9 && field[..7] == [0xff, 0xff, 0xff, b'k', b'b', b'w', 2] {
            side = match field[7] {
                1 => Some((Side::Left, field[8] & DISPLAY_CAPABILITY != 0)),
                2 => Some((Side::Right, false)),
                _ => None,
            };
        }
        data = &data[length + 1..];
    }
    if service { side } else { None }
}

pub(super) async fn peer_loop(stack: &Host<'_>, bonds: &Bonds, side: Side) {
    loop {
        while !crate::settings::bluetooth_enabled() {
            Timer::after_millis(100).await;
        }
        let epoch = crate::settings::radio_epoch();
        let (connection, display_feedback) = {
            let _guard = CONNECT_LOCK.lock().await;
            let mut central = stack.central();
            ADVERTISEMENTS.clear();
            let candidate = {
                let mut scanner = Scanner::new(&mut central);
                let config = ScanConfig {
                    active: false,
                    interval: Duration::from_millis(100),
                    window: Duration::from_millis(100),
                    ..Default::default()
                };
                let Ok(_scan) = scanner.scan(&config).await else {
                    Timer::after_millis(100).await;
                    continue;
                };
                let discovery = async {
                    loop {
                        let candidate = ADVERTISEMENTS.receive().await;
                        if candidate.side == side {
                            break candidate;
                        }
                    }
                };
                match with_timeout(Duration::from_secs(1), discovery).await {
                    Ok(candidate) => candidate,
                    Err(_) => continue,
                }
            };
            Timer::after_millis(20).await;
            let filter = [candidate.address];
            let config = ConnectConfig {
                scan_config: ScanConfig {
                    filter_accept_list: &filter,
                    interval: Duration::from_millis(30),
                    window: Duration::from_millis(30),
                    ..Default::default()
                },
                connect_params: RequestedConnParams {
                    min_connection_interval: Duration::from_micros(7500),
                    max_connection_interval: Duration::from_millis(15),
                    max_latency: 0,
                    min_event_length: Duration::from_millis(0),
                    max_event_length: Duration::from_millis(0),
                    supervision_timeout: Duration::from_secs(2),
                },
            };
            match with_timeout(Duration::from_secs(10), central.connect(&config)).await {
                Ok(Ok(connection)) => (connection, candidate.display_feedback),
                _ => continue,
            }
        };
        if crate::radio::authenticate(&connection, stack, bonds).await {
            let _ = select(
                receive_snapshots(stack, bonds, &connection, side, display_feedback),
                crate::settings::radio_changed(epoch),
            )
            .await;
        }
        connection.disconnect();
        EVENTS.send(LinkEvent::Close(side)).await;
        Timer::after_millis(100).await;
    }
}

async fn receive_snapshots(
    stack: &Host<'_>,
    bonds: &Bonds,
    connection: &Connection<'_, DefaultPacketPool>,
    side: Side,
    display_feedback: bool,
) {
    let Ok(Ok(client)) = with_timeout(
        Duration::from_secs(5),
        GattClient::<_, DefaultPacketPool, 12>::new(stack, connection),
    )
    .await
    else {
        return;
    };
    let receive = async {
        let Ok(Ok(services)) = with_timeout(
            Duration::from_secs(5),
            client.services_by_uuid(&SERVICE_UUID),
        )
        .await
        else {
            return;
        };
        let Some(service) = services.first() else {
            return;
        };
        let result: Result<Result<Characteristic<[u8; PACKET_LEN]>, _>, _> = with_timeout(
            Duration::from_secs(5),
            client.characteristic_by_uuid(service, &SNAPSHOT_UUID),
        )
        .await;
        let Ok(Ok(characteristic)) = result else {
            return;
        };
        let Ok(Ok(mut listener)) = with_timeout(
            Duration::from_secs(5),
            client.subscribe(&characteristic, false),
        )
        .await
        else {
            return;
        };
        let notifications = async {
            let mut opens = true;
            loop {
                let Ok(notification) = with_timeout(Duration::from_secs(1), listener.next()).await
                else {
                    return;
                };
                let Ok(snapshot) = Snapshot::decode(notification.as_ref()) else {
                    continue;
                };
                if snapshot.side != side {
                    return;
                }
                opens = EVENTS
                    .try_send(LinkEvent::snapshot(snapshot, opens))
                    .is_err();
            }
        };
        let events = async {
            loop {
                match connection.next().await {
                    ConnectionEvent::Disconnected { .. } | ConnectionEvent::PairingFailed(_) => {
                        break;
                    }
                    ConnectionEvent::PairingComplete {
                        bond: Some(bond), ..
                    } => {
                        bonds.lock().await.remember(&bond).await;
                    }
                    ConnectionEvent::RequestConnectionParams(request) => {
                        let _ = request.accept(None, stack).await;
                    }
                    _ => {}
                }
            }
        };
        let link = select(events, notifications);
        #[cfg(all(feature = "toucan-dongle", feature = "prospector-display"))]
        if display_feedback {
            let feedback = async {
                let characteristic = with_timeout(
                    Duration::from_secs(1),
                    client
                        .characteristic_by_uuid::<[u8; DISPLAY_PACKET_LEN]>(service, &DISPLAY_UUID),
                )
                .await;
                if let Ok(Ok(characteristic)) = characteristic {
                    let mut sequence = 0u16;
                    loop {
                        let now = Instant::now().as_millis() as u32;
                        if let Some((status, published_at)) = DISPLAY_OUTPUT.lock(Cell::get) {
                            if now.wrapping_sub(published_at) < 500 {
                                let packet = DisplayFeedback { sequence, status }.encode();
                                let _ = with_timeout(
                                    Duration::from_millis(25),
                                    client.write_characteristic_without_response(
                                        &characteristic,
                                        &packet,
                                    ),
                                )
                                .await;
                                sequence = sequence.wrapping_add(1);
                            }
                        }
                        Timer::after_millis(100).await;
                    }
                }
                core::future::pending::<()>().await;
            };
            let _ = select(link, feedback).await;
            return;
        }
        let _ = display_feedback;
        let _ = link.await;
    };
    let _ = select(client.task(), receive).await;
}

async fn poll_matrix(receivers: &mut [Receiver; 2], accept: bool) -> (u32, u64, bool) {
    let event = if accept {
        select(EVENTS.receive(), Timer::after_millis(5)).await
    } else {
        Timer::after_millis(1).await;
        Either::Second(())
    };
    let now = Instant::now().as_millis() as u32;
    let mut invalidated = false;
    for receiver in receivers.iter_mut() {
        invalidated |= receiver.expire(now);
    }
    if let Either::First(event) = event {
        match event {
            LinkEvent::Snapshot {
                snapshot,
                received_at,
                opens,
            } => {
                invalidated |= receivers[snapshot.side.index()]
                    .receive_queued(snapshot, received_at, now, opens)
                    .unwrap_or(false);
            }
            LinkEvent::Close(side) => {
                let receiver = &mut receivers[side.index()];
                invalidated |= receiver.connected();
                receiver.close();
            }
        }
    }
    let keys = receivers
        .iter()
        .fold(0, |keys, receiver| keys | receiver.keys());
    (now, keys, invalidated)
}

#[embassy_executor::task]
pub(super) async fn process_keys() {
    let mut component = KeyboardComponent::new(include_bytes!(env!("KB_KEYBOARD_WASM"))).ok();
    let mut state = Bounded::<u8, 64>::new(0);
    let mut reports = ReportState::<42>::new();
    let mut receivers = [Receiver::new(); 2];
    let mut previous = 0;
    let mut input_gate = InputGate::new();
    let mut wake_at = None;
    let mut display_status = kb_display::Status {
        transport: kb_display::Transport::Split,
        ..Default::default()
    };
    let mut previous_display = display_status;
    loop {
        let (now, keys, invalidated) = poll_matrix(&mut receivers, true).await;
        let session = transport();
        let ready = session.ready();
        let gate = input_gate.observe(session.generation(), ready, keys, invalidated);
        if kb_core::matrix_link::switch_typing_mode(previous, gate.keys) {
            drop(component.take());
            REPORTS.clear();
            crate::usb_hid::invalidate_pending_reports();
            let mut switch_gate = InputGate::new();
            let mut last = keys;
            crate::braille_receiver::run(async |accept| {
                let (_, keys, invalidated) = poll_matrix(&mut receivers, accept).await;
                let session = transport();
                let gate = switch_gate.observe(0, session.ready(), keys, invalidated);
                let exit = kb_core::matrix_link::switch_typing_mode(last, gate.keys);
                let old = kb_core::matrix_link::braille_keys(last);
                last = keys;
                let dots = kb_core::matrix_link::braille_keys(keys);
                let keys = if gate.keys.is_some() { dots } else { 0 };
                (!exit).then_some((
                    kb_core::split::Update {
                        keys,
                        pressed: keys & !old,
                        released: old & !keys,
                        cancel_chord: invalidated,
                    },
                    receivers.iter().filter(|r| r.connected()).count() as u8,
                ))
            })
            .await;
            component = KeyboardComponent::new(include_bytes!(env!("KB_KEYBOARD_WASM"))).ok();
            REPORTS.clear();
            crate::usb_hid::invalidate_pending_reports();
            input_gate = InputGate::new();
            crate::prospector::update(display_status);
            continue;
        }
        if gate.cancel {
            state = Default::default();
            reports.clear();
            wake_at = None;
            previous = 0;
            REPORTS.clear();
            {
                display_status.last_usage = 0;
                display_status.last_modifiers = 0;
            }
            if ready {
                REPORTS
                    .send((session.generation(), crate::usb_hid::Output::Held([0; 8])))
                    .await;
            }
        } else if component.is_some()
            && gate.keys.is_some()
            && (previous != keys
                || wake_at.is_some_and(|deadline: u32| now.wrapping_sub(deadline) as i32 >= 0))
        {
            let mut input = Input::<42>::tick(now);
            for position in 0..42 {
                let mask = 1u64 << position;
                if (previous ^ keys) & mask != 0 {
                    input
                        .events
                        .push(KeyEvent {
                            position,
                            state: if keys & mask != 0 {
                                KeyState::Pressed
                            } else {
                                KeyState::Released
                            },
                            timestamp_ms: now,
                        })
                        .unwrap();
                }
            }
            let result = (|| {
                let component = component
                    .as_ref()
                    .ok_or(kb_component_runtime::Error::InvalidState)?;
                let (next_state, wake, actions) = {
                    let output = component.process::<64, 42, 42>(state.as_slice(), &input)?;
                    let wake = output.wake_at_ms;
                    let output = component.transform_output::<64, 42>(
                        output.state.as_slice(),
                        output.actions.as_slice(),
                    )?;
                    (output.state, wake, output.actions)
                };
                let mut next_reports = reports;
                for action in actions.as_slice() {
                    next_reports
                        .apply(*action)
                        .map_err(|_| kb_component_runtime::Error::InvalidReturn)?;
                }
                Ok::<_, kb_component_runtime::Error>((next_state, wake, next_reports, actions))
            })();
            match result {
                Ok((next_state, wake, next_reports, actions)) => {
                    state = next_state;
                    wake_at = wake;
                    let mut replay = reports;
                    reports = next_reports;
                    for action in actions.as_slice() {
                        let update = replay.apply(*action).unwrap();
                        for report in [Some(update.first), update.second].into_iter().flatten() {
                            REPORTS
                                .send((session.generation(), crate::usb_hid::Output::Held(report)))
                                .await;
                            {
                                display_status.last_modifiers = report[0];
                                if report[2] != 0 {
                                    display_status.last_usage = report[2];
                                }
                            }
                        }
                    }
                }
                Err(_) => {
                    component = None;
                    state = Default::default();
                    wake_at = None;
                    reports.clear();
                    REPORTS.clear();
                    REPORTS
                        .send((session.generation(), crate::usb_hid::Output::Held([0; 8])))
                        .await;
                }
            }
            previous = keys;
        }
        COMPONENT_FAILED.store(component.is_none(), Ordering::Relaxed);
        {
            display_status.connected = ready && receivers.iter().all(Receiver::connected);
            display_status.pairing_unsaved = crate::ble_bonds::storage_failed();
            display_status.faulted = component.is_none();
            let keyboard = kb_display::KeyboardStatus {
                active_keys: keys.count_ones() as u8,
                layer: u8::from(keys & (1u64 << 36) != 0),
            };
            display_status.keyboard = Some(keyboard);
            publish_feedback(display_status, keyboard, now);
            if display_status != previous_display {
                crate::prospector::update(display_status);
                previous_display = display_status;
            }
        }
    }
}

pub(crate) fn publish_feedback(
    status: kb_display::Status,
    keyboard: kb_display::KeyboardStatus,
    now: u32,
) {
    COMPONENT_FAILED.store(status.faulted, Ordering::Relaxed);
    DISPLAY_OUTPUT.lock(|output| {
        output.set(Some((
            DisplayStatus {
                connected: status.connected,
                faulted: status.faulted,
                pairing_unsaved: status.pairing_unsaved,
                active_keys: keyboard.active_keys,
                layer: keyboard.layer,
                last_usage: status.last_usage,
                last_modifiers: status.last_modifiers,
            },
            now,
        )))
    });
}

fn transport() -> SessionGate {
    crate::usb_hid::connection()
}

pub(super) async fn run_usb(peripheral: Peri<'static, peripherals::USBD>) {
    let driver = Driver::new(peripheral, Irqs, HardwareVbusDetect::new(Irqs));
    let mut config = Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("kb.wlt");
    config.product = Some("kb.wlt Toucan Receiver");
    config.max_power = 100;
    config.max_packet_size_0 = 64;
    config.composite_with_iads = false;
    let mut configuration = [0; 256];
    let mut bos = [0; 64];
    let mut msos = [0; 64];
    let mut control = [0; 64];
    let mut state = State::new();
    let mut handler = crate::usb_hid::ConnectionState::default();
    let mut hid_control = crate::usb_hid::KeyboardControl::default();
    let mut consumer_state = State::new();
    let mut consumer_control = crate::usb_hid::ConsumerControl;
    let mut builder = Builder::new(
        driver,
        config,
        &mut configuration,
        &mut bos,
        &mut msos,
        &mut control,
    );
    builder.handler(&mut handler);
    let mut writer = HidWriter::<_, 8>::new(
        &mut builder,
        &mut state,
        HidConfig {
            report_descriptor: &REPORT_DESCRIPTOR,
            request_handler: Some(&mut hid_control),
            poll_ms: 1,
            max_packet_size: 8,
            hid_subclass: HidSubclass::Boot,
            hid_boot_protocol: HidBootProtocol::Keyboard,
        },
    );
    let mut consumer =
        crate::usb_hid::consumer_writer(&mut builder, &mut consumer_state, &mut consumer_control);
    let mut usb = builder.build();
    let send = crate::usb_hid::send_strokes(&mut writer, &mut consumer, &REPORTS);
    join(usb.run(), send).await;
}
