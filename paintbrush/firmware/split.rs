#[cfg(feature = "prospector-display")]
use crate::radio::DisplayIrqs;
use crate::radio::{Bonds, address, device_id, radio};
#[cfg(feature = "prospector-display")]
use crate::radio::{Host, Irqs, authenticate};
use core::sync::atomic::{AtomicU8, Ordering};
use embassy_executor::Spawner;
#[cfg(feature = "prospector-display")]
use embassy_futures::select::Either;
use embassy_futures::{
    join::{join, join3},
    select::select,
};
use embassy_nrf::gpio::{Input, Level, Output, OutputDrive, Pull};
#[cfg(feature = "prospector-display")]
use embassy_nrf::interrupt::InterruptExt;
use embassy_nrf::interrupt::Priority;
#[cfg(feature = "prospector-display")]
use embassy_nrf::usb::{Driver, vbus_detect::HardwareVbusDetect};
use embassy_nrf::{Peri, peripherals};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
#[cfg(feature = "prospector-display")]
use embassy_sync::{channel::Channel, mutex::Mutex};
#[cfg(feature = "prospector-display")]
use embassy_time::{Duration, with_timeout};
use embassy_time::{Instant, Timer};
#[cfg(feature = "prospector-display")]
use embassy_usb::class::hid::{
    Config as HidConfig, HidBootProtocol, HidSubclass, HidWriter, State,
};
#[cfg(feature = "prospector-display")]
use embassy_usb::{Builder, Config};
use kb_core::Debouncer;
use kb_core::split::{Layout, PACKET_LEN, Snapshot};
#[cfg(feature = "prospector-display")]
use kb_core::{
    hid::KEYBOARD_REPORT_DESCRIPTOR as REPORT_DESCRIPTOR,
    split::{Receiver, Session},
};
use nrf_sdc::{self as sdc, mpsl};
use trouble_host::prelude::*;

const SERVICE_BYTES: [u8; 16] = [
    0x43, 0x02, 0x12, 0x11, 0x4b, 0x57, 0xa7, 0x98, 0x6c, 0xaa, 0x03, 0xaf, 0x11, 0x00, 0x17, 0x7a,
];
const SERVICE_UUID: Uuid = Uuid::new_long(SERVICE_BYTES);
const SNAPSHOT_UUID: Uuid = Uuid::new_long([
    0x44, 0x02, 0x12, 0x11, 0x4b, 0x57, 0xa7, 0x98, 0x6c, 0xaa, 0x03, 0xaf, 0x11, 0x00, 0x17, 0x7a,
]);

static KEYS: AtomicU8 = AtomicU8::new(0);
static KEY_CHANGED: Signal<CriticalSectionRawMutex, ()> = Signal::new();
#[cfg(feature = "prospector-display")]
const LINK_TIMEOUT_MS: u32 = 500;
#[cfg(feature = "prospector-display")]
static EVENTS: Channel<CriticalSectionRawMutex, (u32, LinkEvent), 64> = Channel::new();
#[cfg(feature = "prospector-display")]
static ADVERTISEMENTS: Channel<CriticalSectionRawMutex, Discovered, 16> = Channel::new();
#[cfg(feature = "prospector-display")]
static CONNECT_LOCK: Mutex<CriticalSectionRawMutex, ()> = Mutex::new(());

#[cfg(feature = "prospector-display")]
#[derive(Clone, Copy)]
struct Discovered {
    address: Address,
    layout: Layout,
}

#[cfg(feature = "prospector-display")]
#[derive(Clone, Copy)]
enum LinkEvent {
    Open(usize, Snapshot),
    Snapshot(usize, Snapshot),
    Close(usize),
}

#[cfg(feature = "prospector-display")]
impl LinkEvent {
    async fn send(self) {
        EVENTS.send((Instant::now().as_millis() as u32, self)).await;
    }
}

#[cfg(feature = "prospector-display")]
struct Discovery;

#[cfg(feature = "prospector-display")]
impl EventHandler for Discovery {
    fn on_adv_reports(&self, mut reports: LeAdvReportsIter<'_>) {
        while let Some(Ok(report)) = reports.next() {
            if let Some(layout) = advertised_layout(report.data) {
                let _ = ADVERTISEMENTS.try_send(Discovered {
                    address: Address::new(report.addr_kind, report.addr),
                    layout,
                });
            }
        }
    }
}

#[cfg(feature = "prospector-display")]
fn advertised_layout(mut data: &[u8]) -> Option<Layout> {
    let mut service = false;
    let mut layout = None;
    while !data.is_empty() {
        let len = data[0] as usize;
        if len == 0 {
            break;
        }
        if len >= data.len() {
            return None;
        }
        let field = &data[1..=len];
        if field[0] == 0x07
            && field[1..]
                .chunks_exact(16)
                .any(|uuid| uuid == SERVICE_BYTES)
        {
            service = true;
        }
        if field.len() == 9 && field[..7] == [0xff, 0xff, 0xff, b'k', b'b', b'w', 1] {
            layout = match field[7] {
                1 => Some(Layout::Left),
                2 => Some(Layout::Right),
                _ => None,
            };
        }
        data = &data[len + 1..];
    }
    if service { layout } else { None }
}

#[gatt_server(connections_max = 1, attribute_table_size = 32)]
struct SatelliteServer {
    keys: KeyService,
}

#[gatt_service(uuid = SERVICE_UUID)]
struct KeyService {
    #[characteristic(uuid = SNAPSHOT_UUID, read, notify, permissions(encrypted))]
    snapshot: [u8; PACKET_LEN],
}

fn inputs(
    p0_31: Peri<'static, peripherals::P0_31>,
    p0_29: Peri<'static, peripherals::P0_29>,
    p0_02: Peri<'static, peripherals::P0_02>,
    p1_15: Peri<'static, peripherals::P1_15>,
    p1_13: Peri<'static, peripherals::P1_13>,
    p1_11: Peri<'static, peripherals::P1_11>,
    p0_10: Peri<'static, peripherals::P0_10>,
    p0_09: Peri<'static, peripherals::P0_09>,
) -> [Input<'static>; 8] {
    [
        Input::new(p0_31, Pull::Up),
        Input::new(p0_29, Pull::Up),
        Input::new(p0_02, Pull::Up),
        Input::new(p1_15, Pull::Up),
        Input::new(p1_13, Pull::Up),
        Input::new(p1_11, Pull::Up),
        Input::new(p0_10, Pull::Up),
        Input::new(p0_09, Pull::Up),
    ]
}

async fn scan(inputs: [Input<'static>; 8], mut indicator: Output<'static>) {
    let mut debouncer = Debouncer::<8>::new(5);
    loop {
        let raw = inputs
            .iter()
            .enumerate()
            .fold(0, |mask, (position, input)| {
                mask | (u64::from(input.is_low()) << position)
            });
        let (next, update) = debouncer.sample(raw, Instant::now().as_millis() as u32);
        debouncer = next;
        let keys = update.stable as u8;
        KEYS.store(keys, Ordering::Relaxed);
        if update.pressed | update.released != 0 {
            KEY_CHANGED.signal(());
        }
        let illuminated = if crate::ble_bonds::storage_failed() {
            let phase = Instant::now().as_millis() % 2000;
            phase < 100 || (200..300).contains(&phase)
        } else {
            keys != 0
        };
        indicator.set_level(if illuminated { Level::High } else { Level::Low });
        Timer::after_millis(1).await;
    }
}

pub async fn satellite(spawner: Spawner) {
    let mut config = crate::clock_config();
    config.time_interrupt_priority = Priority::P3;
    let p = embassy_nrf::init(config);
    let inputs = inputs(
        p.P0_31, p.P0_29, p.P0_02, p.P1_15, p.P1_13, p.P1_11, p.P0_10, p.P0_09,
    );
    let indicator = Output::new(p.P0_15, Level::Low, OutputDrive::Standard);
    let (controller, mut bonds, boot) = radio(
        spawner,
        mpsl::Peripherals::new(p.RTC0, p.TIMER0, p.TEMP, p.PPI_CH19, p.PPI_CH30, p.PPI_CH31),
        sdc::Peripherals::new(
            p.PPI_CH17, p.PPI_CH18, p.PPI_CH20, p.PPI_CH21, p.PPI_CH22, p.PPI_CH23, p.PPI_CH24,
            p.PPI_CH25, p.PPI_CH26, p.PPI_CH27, p.PPI_CH28, p.PPI_CH29,
        ),
        p.RNG,
        p.NVMC,
        0,
        1,
    );
    if inputs.iter().all(Input::is_low) {
        Timer::after_secs(2).await;
        if inputs.iter().all(Input::is_low) {
            bonds.forget().await;
        }
    }
    let saved = bonds.load_all().unwrap();
    let mut resources: HostResources<DefaultPacketPool, 1, 3> = HostResources::new();
    let stack = trouble_host::new(controller, &mut resources)
        .set_random_address(address())
        .build();
    for bond in saved.into_iter().flatten() {
        stack.add_bond_information(bond).unwrap();
    }
    let bonds = Bonds::new(bonds);
    let mut runner = stack.runner();
    let mut peripheral = stack.peripheral();
    let server = SatelliteServer::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: "kb.wlt satellite",
        appearance: &appearance::human_interface_device::KEYBOARD,
    }))
    .unwrap();
    let layout = if cfg!(feature = "satellite-left") {
        Layout::Left
    } else {
        Layout::Right
    };
    let connections = async {
        let mut advertising = [0; 31];
        let len = AdStructure::encode_slice(
            &[
                AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
                AdStructure::CompleteServiceUuids128(&[SERVICE_BYTES]),
                AdStructure::ManufacturerSpecificData {
                    company_identifier: 0xffff,
                    payload: &[b'k', b'b', b'w', 1, layout as u8, 0],
                },
            ],
            &mut advertising,
        )
        .unwrap();
        loop {
            let advertisement = peripheral
                .advertise(
                    &Default::default(),
                    Advertisement::ConnectableScannableUndirected {
                        adv_data: &advertising[..len],
                        scan_data: &[],
                    },
                )
                .await;
            let Ok(advertisement) = advertisement else {
                Timer::after_millis(100).await;
                continue;
            };
            let Ok(connection) = advertisement.accept().await else {
                continue;
            };
            if connection.set_bondable(true).is_err() {
                continue;
            }
            let connection = connection.with_attribute_server(&server).unwrap();
            let notify = async {
                let mut sequence = 0u16;
                loop {
                    if connection
                        .raw()
                        .security_level()
                        .map(|s| s.encrypted())
                        .unwrap_or(false)
                        && server.keys.snapshot.should_notify(&connection)
                    {
                        let snapshot = Snapshot {
                            node: device_id(),
                            boot,
                            sequence,
                            layout,
                            battery: crate::battery::level(),
                            keys: KEYS.load(Ordering::Relaxed),
                        };
                        if server
                            .keys
                            .snapshot
                            .notify(&connection, &snapshot.encode(), true)
                            .await
                            .is_err()
                        {
                            break;
                        }
                        sequence = sequence.wrapping_add(1);
                    }
                    let _ = select(KEY_CHANGED.wait(), Timer::after_millis(100)).await;
                }
            };
            let events = async {
                loop {
                    match connection.next().await {
                        GattConnectionEvent::Disconnected { .. } => break,
                        GattConnectionEvent::PairingComplete {
                            bond: Some(bond), ..
                        } => {
                            let mut bonds = bonds.lock().await;
                            bonds.remember(&bond).await;
                            bonds
                                .remember_client_table(
                                    &server.server,
                                    connection.raw(),
                                    crate::ble_bonds::PAINTBRUSH_LINK_SCHEMA,
                                )
                                .await;
                        }
                        GattConnectionEvent::Encrypted { security_level, .. }
                            if security_level.encrypted() =>
                        {
                            bonds.lock().await.restore_client_table(
                                &server.server,
                                connection.raw(),
                                crate::ble_bonds::PAINTBRUSH_LINK_SCHEMA,
                            );
                        }
                        GattConnectionEvent::Gatt { event } => {
                            let changed = matches!(&event, GattEvent::Write(_));
                            if let Ok(reply) = event.accept() {
                                reply.send().await;
                            }
                            if changed {
                                bonds
                                    .lock()
                                    .await
                                    .remember_client_table(
                                        &server.server,
                                        connection.raw(),
                                        crate::ble_bonds::PAINTBRUSH_LINK_SCHEMA,
                                    )
                                    .await;
                            }
                        }
                        _ => {}
                    }
                }
            };
            let _ = select(events, notify).await;
        }
    };
    let battery = crate::battery::monitor(
        p.SAADC,
        embassy_nrf::saadc::ChannelConfig::single_ended(embassy_nrf::saadc::VddhDiv5Input),
        5,
        1,
    );
    join3(
        runner.run(),
        connections,
        join(scan(inputs, indicator), battery),
    )
    .await;
}

#[cfg(feature = "prospector-display")]
pub async fn central(spawner: Spawner) {
    let mut config = crate::clock_config();
    config.time_interrupt_priority = Priority::P3;
    let p = embassy_nrf::init(config);
    let display = {
        embassy_nrf::interrupt::SPI2.set_priority(Priority::P3);
        let mut spi_config = embassy_nrf::spim::Config::default();
        spi_config.frequency = embassy_nrf::spim::Frequency::M4;
        let spi =
            embassy_nrf::spim::Spim::new_txonly(p.SPI2, DisplayIrqs, p.P1_13, p.P1_15, spi_config);
        crate::prospector::run(
            spi,
            Output::new(p.P1_14, Level::High, OutputDrive::Standard),
            Output::new(p.P1_12, Level::High, OutputDrive::Standard),
            Output::new(p.P0_29, Level::High, OutputDrive::Standard),
            Output::new(p.P1_11, Level::Low, OutputDrive::Standard),
        )
    };
    let (controller, mut bonds, _) = radio(
        spawner,
        mpsl::Peripherals::new(p.RTC0, p.TIMER0, p.TEMP, p.PPI_CH19, p.PPI_CH30, p.PPI_CH31),
        sdc::Peripherals::new(
            p.PPI_CH17, p.PPI_CH18, p.PPI_CH20, p.PPI_CH21, p.PPI_CH22, p.PPI_CH23, p.PPI_CH24,
            p.PPI_CH25, p.PPI_CH26, p.PPI_CH27, p.PPI_CH28, p.PPI_CH29,
        ),
        p.RNG,
        p.NVMC,
        2,
        0,
    );
    let saved = bonds.load_all().unwrap();
    let mut config_bytes = [0; 512];
    if let Ok(Some(length)) = bonds.load_config(&mut config_bytes) {
        crate::settings::initialize(Some(&config_bytes[..length]));
    }
    let mut resources: HostResources<DefaultPacketPool, 2, 6> = HostResources::new();
    let stack = trouble_host::new(controller, &mut resources)
        .set_random_address(address())
        .build();
    for bond in saved.into_iter().flatten() {
        stack.add_bond_information(bond).unwrap();
    }
    let bonds = Bonds::new(bonds);
    let mut runner = stack.runner();
    let links = join(
        peer_loop(&stack, &bonds, 0, Layout::Left),
        peer_loop(&stack, &bonds, 1, Layout::Right),
    );
    let application = join(process_keys(), run_usb(p.USBD));
    let settings = join(
        crate::settings::run_saves(&bonds),
        crate::settings::run_controls(&stack, &bonds),
    );
    let keyboard = join3(
        runner.run_with_handler(&Discovery),
        links,
        join(application, settings),
    );
    let _ = join(keyboard, display).await;
}

#[cfg(feature = "prospector-display")]
async fn peer_loop(stack: &Host<'_>, bonds: &Bonds, slot: usize, layout: Layout) {
    loop {
        while !crate::settings::bluetooth_enabled() {
            Timer::after_millis(50).await;
        }
        let epoch = crate::settings::radio_epoch();
        let connect = async {
            loop {
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
                            if candidate.layout == layout {
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
                    Ok(Ok(connection)) => break connection,
                    _ => continue,
                }
            }
        };
        let connection = match select(connect, crate::settings::radio_changed(epoch)).await {
            Either::First(connection) => connection,
            Either::Second(_) => continue,
        };
        let connected = async {
            if authenticate(&connection, stack, bonds).await {
                receive_snapshots(stack, bonds, &connection, slot, layout).await;
            }
        };
        let _ = select(connected, crate::settings::radio_changed(epoch)).await;
        connection.disconnect();
        LinkEvent::Close(slot).send().await;
        Timer::after_millis(100).await;
    }
}

#[cfg(feature = "prospector-display")]
async fn receive_snapshots(
    stack: &Host<'_>,
    bonds: &Bonds,
    connection: &Connection<'_, DefaultPacketPool>,
    slot: usize,
    layout: Layout,
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
            let mut first = true;
            loop {
                let Ok(notification) = with_timeout(Duration::from_secs(1), listener.next()).await
                else {
                    return;
                };
                let Ok(snapshot) = Snapshot::decode(notification.as_ref()) else {
                    continue;
                };
                if snapshot.layout != layout {
                    return;
                }
                let event = if first {
                    LinkEvent::Open(slot, snapshot)
                } else {
                    LinkEvent::Snapshot(slot, snapshot)
                };
                event.send().await;
                first = false;
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
        let _ = select(events, notifications).await;
    };
    let _ = select(client.task(), receive).await;
}

#[cfg(feature = "prospector-display")]
async fn process_keys() {
    let mut receiver = Receiver::<2>::new(LINK_TIMEOUT_MS);
    let mut sessions: [Option<Session>; 2] = [None; 2];
    let mut client = kb_component_runtime::BrailleComponent::new_embedded().ok();
    crate::braille_receiver::run(&mut client, async |accept| {
        let now = Instant::now().as_millis() as u32;
        let (next, mut update) = receiver.expire(now);
        receiver = next;
        let event = if accept {
            match select(EVENTS.receive(), Timer::after_millis(5)).await {
                Either::First(event) => Some(event),
                Either::Second(_) => None,
            }
        } else {
            Timer::after_millis(1).await;
            None
        };
        let now = Instant::now().as_millis() as u32;
        if let Some((received_at, event)) = event.filter(|(received_at, event)| {
            matches!(event, LinkEvent::Close(_)) || now.wrapping_sub(*received_at) < LINK_TIMEOUT_MS
        }) {
            let result = match event {
                LinkEvent::Open(slot, snapshot) => {
                    let (next, session, opened) = receiver
                        .connect(slot, snapshot.node, snapshot.layout, received_at)
                        .unwrap();
                    receiver = next;
                    sessions[slot] = Some(session);
                    update.cancel_chord |= opened.cancel_chord;
                    receiver.receive(session, snapshot, received_at).ok()
                }
                LinkEvent::Snapshot(slot, snapshot) => {
                    if !receiver.is_connected(slot) {
                        let (next, session, opened) = receiver
                            .connect(slot, snapshot.node, snapshot.layout, received_at)
                            .unwrap();
                        receiver = next;
                        update.cancel_chord |= opened.cancel_chord;
                        sessions[slot] = Some(session);
                    }
                    sessions[slot]
                        .and_then(|session| receiver.receive(session, snapshot, received_at).ok())
                }
                LinkEvent::Close(slot) => sessions[slot]
                    .take()
                    .and_then(|session| receiver.disconnect(session).ok()),
            };
            if let Some((next, received)) = result {
                update.cancel_chord |= received.cancel_chord;
                update.keys = received.keys;
                update.pressed |= received.pressed;
                update.released |= received.released;
                receiver = next;
            }
        }
        Some((
            update,
            kb_client_session::Links {
                online: u8::from(receiver.is_connected(0))
                    | (u8::from(receiver.is_connected(1)) << 1),
                battery: [receiver.battery(0), receiver.battery(1)],
            },
        ))
    })
    .await;
}

#[cfg(feature = "prospector-display")]
async fn run_usb(peripheral: Peri<'static, peripherals::USBD>) {
    let driver = Driver::new(peripheral, Irqs, HardwareVbusDetect::new(Irqs));
    let mut config = Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("kb.wlt");
    config.product = Some("kb.wlt Split Receiver");
    config.max_power = 100;
    config.max_packet_size_0 = 64;
    config.composite_with_iads = false;
    let mut configuration = [0; 256];
    let mut bos = [0; 64];
    let mut msos = [0; 64];
    let mut control = [0; 64];
    let mut state = State::new();
    let mut consumer_state = State::new();
    let mut consumer_control = crate::usb_hid::ConsumerControl;
    let mut control_handler = crate::usb_hid::KeyboardControl::default();
    let mut handler = crate::usb_hid::ConnectionState::default();
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
            request_handler: Some(&mut control_handler),
            poll_ms: 1,
            max_packet_size: 8,
            hid_subclass: HidSubclass::Boot,
            hid_boot_protocol: HidBootProtocol::Keyboard,
        },
    );
    let mut consumer =
        crate::usb_hid::consumer_writer(&mut builder, &mut consumer_state, &mut consumer_control);
    let mut usb = builder.build();
    let reports =
        crate::usb_hid::send_strokes(&mut writer, &mut consumer, &crate::braille_receiver::OUTPUT);
    join(usb.run(), reports).await;
}
