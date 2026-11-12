#[cfg(feature = "prospector-display")]
use crate::split::DisplayIrqs;
use crate::split::{Bonds, Host, Irqs, address, device_id};
use core::cell::Cell;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use embassy_executor::Spawner;
use embassy_futures::join::{join, join3};
use embassy_futures::select::{Either, select};
#[cfg(not(feature = "toucan-dongle"))]
use embassy_nrf::gpio::Pull;
use embassy_nrf::gpio::{Input as GpioInput, Level, Output as GpioOutput, OutputDrive};
#[cfg(any(feature = "nice-view", feature = "prospector-display"))]
use embassy_nrf::interrupt::InterruptExt;
use embassy_nrf::interrupt::Priority;
use embassy_nrf::usb::{Driver, vbus_detect::HardwareVbusDetect};
use embassy_nrf::{Peri, peripherals};
use embassy_sync::blocking_mutex::Mutex as BlockingMutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Instant, Timer, with_timeout};
use embassy_usb::class::hid::{
    Config as HidConfig, HidBootProtocol, HidSubclass, HidWriter, State,
};
use embassy_usb::{Builder, Config};
use kb_component_runtime::KeyboardComponent;
use kb_core::Debouncer;
use kb_core::hid::KEYBOARD_REPORT_DESCRIPTOR as REPORT_DESCRIPTOR;
use kb_core::matrix_link::{KEYS_MASK, PACKET_LEN, Receiver, Side, Snapshot};
use kb_core::pipeline::{Bounded, Input, KeyEvent, KeyState};
use kb_core::report_state::ReportState;
use kb_core::transport::{InputGate, SessionGate};
use nrf_sdc::{self as sdc, mpsl};
use trouble_host::prelude::*;

#[cfg(feature = "nice-view")]
embassy_nrf::bind_interrupts!(struct DisplayIrqs {
    SPIM3 => embassy_nrf::spim::InterruptHandler<peripherals::SPI3>;
});

const SERVICE_BYTES: [u8; 16] = [
    0x43, 0x02, 0x12, 0x11, 0x4b, 0x57, 0xa7, 0x98, 0x6c, 0xaa, 0x03, 0xaf, 0x22, 0x00, 0x17, 0x7a,
];
const SERVICE_UUID: Uuid = Uuid::new_long(SERVICE_BYTES);
const SNAPSHOT_UUID: Uuid = Uuid::new_long([
    0x44, 0x02, 0x12, 0x11, 0x4b, 0x57, 0xa7, 0x98, 0x6c, 0xaa, 0x03, 0xaf, 0x22, 0x00, 0x17, 0x7a,
]);

static BLE_SESSION: BlockingMutex<CriticalSectionRawMutex, Cell<SessionGate>> =
    BlockingMutex::new(Cell::new(SessionGate::new()));
static COMPONENT_FAILED: AtomicBool = AtomicBool::new(false);
static KEYS: AtomicU32 = AtomicU32::new(0);
static KEY_CHANGED: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static REPORTS: Channel<CriticalSectionRawMutex, (u32, [u8; 8]), 64> = Channel::new();
static EVENTS: Channel<CriticalSectionRawMutex, LinkEvent, 64> = Channel::new();
static ADVERTISEMENTS: Channel<CriticalSectionRawMutex, Discovered, 16> = Channel::new();
static CONNECT_LOCK: Mutex<CriticalSectionRawMutex, ()> = Mutex::new(());

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Keyboard,
    Satellite,
    Dongle,
}

#[derive(Clone, Copy)]
struct Discovered {
    address: Address,
    side: Side,
}

#[derive(Clone, Copy)]
enum LinkEvent {
    Open(usize, Snapshot),
    Snapshot(usize, Snapshot),
    Close(usize),
}

struct Discovery;

impl EventHandler for Discovery {
    fn on_adv_reports(&self, mut reports: LeAdvReportsIter<'_>) {
        while let Some(Ok(report)) = reports.next() {
            if let Some(side) = advertised_side(report.data) {
                let _ = ADVERTISEMENTS.try_send(Discovered {
                    address: Address::new(report.addr_kind, report.addr),
                    side,
                });
            }
        }
    }
}

fn advertised_side(mut data: &[u8]) -> Option<Side> {
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
                1 => Some(Side::Left),
                2 => Some(Side::Right),
                _ => None,
            };
        }
        data = &data[length + 1..];
    }
    if service { side } else { None }
}

mod satellite_gatt {
    use super::*;
    #[gatt_server(connections_max = 1, attribute_table_size = 32)]
    pub(super) struct SatelliteServer {
        pub(super) keys: KeyService,
    }

    #[gatt_service(uuid = SERVICE_UUID)]
    pub(super) struct KeyService {
        #[characteristic(uuid = SNAPSHOT_UUID, read, notify, permissions(encrypted))]
        pub(super) snapshot: [u8; PACKET_LEN],
    }
}
use satellite_gatt::SatelliteServer;

struct Matrix {
    rows: [GpioInput<'static>; 4],
    columns: [GpioOutput<'static>; 6],
    side: Side,
}

impl Matrix {
    async fn read(&mut self) -> u64 {
        let mut keys = 0;
        for column in 0..6 {
            self.columns[column].set_high();
            Timer::after_micros(30).await;
            for row in 0..4 {
                if self.rows[row].is_high() {
                    if let Some(index) = self.side.matrix_index(row as u8, column as u8) {
                        keys |= 1 << index;
                    }
                }
            }
            self.columns[column].set_low();
        }
        keys
    }

    async fn scan(mut self) {
        let mut debounce = Debouncer::<21>::new(5);
        loop {
            let raw = self.read().await;
            let (next, update) = debounce.sample(raw, Instant::now().as_millis() as u32);
            debounce = next;
            KEYS.store(update.stable as u32, Ordering::Relaxed);
            if update.pressed | update.released != 0 {
                KEY_CHANGED.signal(());
            }
            Timer::after_millis(1).await;
        }
    }
}

pub async fn run(spawner: Spawner, role: Role) {
    if role != Role::Satellite {
        crate::init_heap();
    }
    let mut config = crate::clock_config();
    config.time_interrupt_priority = Priority::P3;
    let p = embassy_nrf::init(config);
    let mut fault_led = GpioOutput::new(p.P0_26, Level::High, OutputDrive::Standard);
    let diagnostic = async {
        loop {
            let phase = Instant::now().as_millis();
            let illuminated = if COMPONENT_FAILED.load(Ordering::Relaxed) {
                phase / 200 % 2 == 0
            } else if crate::ble_bonds::storage_failed() {
                phase % 2000 < 100 || (200..300).contains(&(phase % 2000))
            } else {
                false
            };
            fault_led.set_level(if illuminated { Level::Low } else { Level::High });
            Timer::after_millis(25).await;
        }
    };
    let side = if cfg!(feature = "toucan-right") {
        Side::Right
    } else {
        Side::Left
    };
    #[cfg(not(feature = "toucan-dongle"))]
    let mut matrix = {
        let mut columns = [
            GpioOutput::new(p.P0_09, Level::Low, OutputDrive::Standard),
            GpioOutput::new(p.P0_04, Level::Low, OutputDrive::Standard),
            GpioOutput::new(p.P0_10, Level::Low, OutputDrive::Standard),
            GpioOutput::new(p.P0_05, Level::Low, OutputDrive::Standard),
            GpioOutput::new(p.P1_11, Level::Low, OutputDrive::Standard),
            GpioOutput::new(p.P1_12, Level::Low, OutputDrive::Standard),
        ];
        if side == Side::Right {
            columns.reverse();
        }
        Some(Matrix {
            rows: [
                GpioInput::new(p.P0_19, Pull::Down),
                GpioInput::new(p.P0_28, Pull::Down),
                GpioInput::new(p.P1_01, Pull::Down),
                GpioInput::new(p.P0_29, Pull::Down),
            ],
            columns,
            side,
        })
    };
    #[cfg(feature = "toucan-dongle")]
    let mut matrix: Option<Matrix> = None;
    #[cfg(feature = "nice-view")]
    let display = {
        embassy_nrf::interrupt::SPIM3.set_priority(Priority::P3);
        let mut config = embassy_nrf::spim::Config::default();
        config.frequency = embassy_nrf::spim::Frequency::M1;
        config.bit_order = embassy_nrf::spim::BitOrder::LsbFirst;
        let spi =
            embassy_nrf::spim::Spim::new_txonly(p.SPI3, DisplayIrqs, p.P1_13, p.P1_15, config);
        crate::nice_view::run(
            spi,
            GpioOutput::new(p.P0_03, Level::Low, OutputDrive::Standard),
        )
    };
    #[cfg(feature = "prospector-display")]
    let display = {
        embassy_nrf::interrupt::SPIM3.set_priority(Priority::P3);
        let mut config = embassy_nrf::spim::Config::default();
        config.frequency = embassy_nrf::spim::Frequency::M16;
        let spi =
            embassy_nrf::spim::Spim::new_txonly(p.SPI3, DisplayIrqs, p.P1_13, p.P1_15, config);
        crate::prospector::run(
            spi,
            GpioOutput::new(p.P1_14, Level::High, OutputDrive::Standard),
            GpioOutput::new(p.P1_12, Level::High, OutputDrive::Standard),
            GpioOutput::new(p.P0_29, Level::High, OutputDrive::Standard),
            GpioOutput::new(p.P1_11, Level::High, OutputDrive::Standard),
        )
    };
    let (central_count, peripheral_count) = match role {
        Role::Keyboard => (1, 1),
        Role::Satellite => (0, 1),
        Role::Dongle => (2, 0),
    };
    let (controller, mut bonds, boot) = crate::split::radio(
        spawner,
        mpsl::Peripherals::new(p.RTC0, p.TIMER0, p.TEMP, p.PPI_CH19, p.PPI_CH30, p.PPI_CH31),
        sdc::Peripherals::new(
            p.PPI_CH17, p.PPI_CH18, p.PPI_CH20, p.PPI_CH21, p.PPI_CH22, p.PPI_CH23, p.PPI_CH24,
            p.PPI_CH25, p.PPI_CH26, p.PPI_CH27, p.PPI_CH28, p.PPI_CH29,
        ),
        p.RNG,
        p.NVMC,
        central_count,
        peripheral_count,
    );
    if let Some(matrix) = matrix.as_mut() {
        if matrix.read().await == KEYS_MASK as u64 {
            Timer::after_secs(2).await;
            if matrix.read().await == KEYS_MASK as u64 {
                bonds.forget().await;
            }
        }
    }
    let saved = bonds.load_all().unwrap();
    let mut resources: HostResources<DefaultPacketPool, 2, 6> = HostResources::new();
    let stack = trouble_host::new(controller, &mut resources)
        .set_random_address(address())
        .set_io_capabilities(IoCapabilities::NoInputNoOutput)
        .build();
    for bond in saved.into_iter().flatten() {
        stack.add_bond_information(bond).unwrap();
    }
    let bonds = Bonds::new(bonds);
    let mut runner = stack.runner();
    let scanner = async {
        if let Some(matrix) = matrix {
            matrix.scan().await;
        } else {
            core::future::pending::<()>().await;
        }
    };
    let application = async {
        match role {
            Role::Satellite => {
                join(satellite(&stack, &bonds, side, boot), scanner).await;
            }
            Role::Keyboard => {
                join3(
                    join(
                        hid_host(&stack, &bonds),
                        peer_loop(&stack, &bonds, 1, Side::Right),
                    ),
                    join(process_keys(), local_snapshots(boot)),
                    scanner,
                )
                .await;
            }
            Role::Dongle => {
                join3(
                    join(
                        peer_loop(&stack, &bonds, 0, Side::Left),
                        peer_loop(&stack, &bonds, 1, Side::Right),
                    ),
                    process_keys(),
                    run_usb(p.USBD),
                )
                .await;
            }
        }
    };
    let keyboard = join3(runner.run_with_handler(&Discovery), application, diagnostic);
    #[cfg(any(feature = "nice-view", feature = "prospector-display"))]
    let _ = join(keyboard, display).await;
    #[cfg(not(any(feature = "nice-view", feature = "prospector-display")))]
    let _ = keyboard.await;
}

async fn satellite(stack: &Host<'_>, bonds: &Bonds, side: Side, boot: u32) {
    let mut peripheral = stack.peripheral();
    let server = SatelliteServer::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: "kb.wlt Toucan half",
        appearance: &appearance::human_interface_device::KEYBOARD,
    }))
    .unwrap();
    let mut advertising = [0; 31];
    let length = AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::CompleteServiceUuids128(&[SERVICE_BYTES]),
            AdStructure::ManufacturerSpecificData {
                company_identifier: 0xffff,
                payload: &[b'k', b'b', b'w', 2, side as u8, 0],
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
                    adv_data: &advertising[..length],
                    scan_data: &[],
                },
            )
            .await;
        let Ok(advertisement) = advertisement else {
            Timer::after_millis(100).await;
            continue;
        };
        let Ok(connection) = advertisement.accept().await else {
            Timer::after_millis(100).await;
            continue;
        };
        if connection.set_bondable(true).is_err() {
            continue;
        }
        let Ok(connection) = connection.with_attribute_server(&server) else {
            continue;
        };
        let notify = async {
            let mut sequence = 0;
            loop {
                if connection
                    .raw()
                    .security_level()
                    .map(|level| level.encrypted())
                    .unwrap_or(false)
                    && server.keys.snapshot.should_notify(&connection)
                {
                    let snapshot = Snapshot {
                        node: device_id(),
                        boot,
                        sequence,
                        side,
                        keys: KEYS.load(Ordering::Relaxed),
                    };
                    if server
                        .keys
                        .snapshot
                        .notify(&connection, &snapshot.encode(), true)
                        .await
                        .is_err()
                    {
                        return;
                    }
                    sequence = sequence.wrapping_add(1);
                }
                let _ = select(KEY_CHANGED.wait(), Timer::after_millis(100)).await;
            }
        };
        let _ = select(
            gatt_events(
                &server.server,
                crate::ble_bonds::TOUCAN_LINK_SCHEMA,
                &connection,
                bonds,
                None,
            ),
            notify,
        )
        .await;
    }
}

async fn local_snapshots(boot: u32) {
    let mut sequence = 0;
    let mut first = true;
    loop {
        let snapshot = Snapshot {
            node: device_id(),
            boot,
            sequence,
            side: Side::Left,
            keys: KEYS.load(Ordering::Relaxed),
        };
        EVENTS
            .send(if first {
                LinkEvent::Open(0, snapshot)
            } else {
                LinkEvent::Snapshot(0, snapshot)
            })
            .await;
        first = false;
        sequence = sequence.wrapping_add(1);
        let _ = select(KEY_CHANGED.wait(), Timer::after_millis(100)).await;
    }
}
async fn peer_loop(stack: &Host<'_>, bonds: &Bonds, slot: usize, side: Side) {
    loop {
        let connection = {
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
                Ok(Ok(connection)) => connection,
                _ => continue,
            }
        };
        if crate::split::authenticate(&connection, stack, bonds).await {
            receive_snapshots(stack, bonds, &connection, slot, side).await;
        }
        connection.disconnect();
        EVENTS.send(LinkEvent::Close(slot)).await;
        Timer::after_millis(100).await;
    }
}

async fn receive_snapshots(
    stack: &Host<'_>,
    bonds: &Bonds,
    connection: &Connection<'_, DefaultPacketPool>,
    slot: usize,
    side: Side,
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
                if snapshot.side != side {
                    return;
                }
                EVENTS
                    .send(if first {
                        LinkEvent::Open(slot, snapshot)
                    } else {
                        LinkEvent::Snapshot(slot, snapshot)
                    })
                    .await;
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

#[gatt_server(connections_max = 1, attribute_table_size = 40)]
struct HidServer {
    hid: HidService,
    _device: DeviceInformation,
}

#[gatt_service(uuid = "1812")]
struct HidService {
    #[characteristic(uuid = "2a4a", read, value = [0x11, 0x01, 0x00, 0x02], permissions(encrypted))]
    information: [u8; 4],
    #[characteristic(uuid = "2a4b", read, value = REPORT_DESCRIPTOR, permissions(encrypted))]
    report_map: [u8; 65],
    #[characteristic(
        uuid = "2a4e",
        read,
        write_without_response,
        value = 1,
        permissions(encrypted)
    )]
    protocol_mode: u8,
    #[characteristic(uuid = "2a4c", write_without_response, permissions(encrypted))]
    control_point: u8,
    #[descriptor(uuid = "2908", read, value = [0u8, 1u8])]
    #[characteristic(uuid = "2a4d", read, notify, permissions(encrypted))]
    input_report: [u8; 8],
    #[descriptor(uuid = "2908", read, value = [0u8, 2u8])]
    #[characteristic(
        uuid = "2a4d",
        read,
        write,
        write_without_response,
        permissions(encrypted)
    )]
    output_report: [u8; 1],
    #[characteristic(uuid = "2a22", read, notify, permissions(encrypted))]
    boot_input: [u8; 8],
    #[characteristic(
        uuid = "2a32",
        read,
        write,
        write_without_response,
        permissions(encrypted)
    )]
    boot_output: [u8; 1],
}

#[gatt_service(uuid = "180a")]
struct DeviceInformation {
    #[characteristic(uuid = "2a29", read, value = *b"kb.wlt")]
    manufacturer: [u8; 6],
    #[characteristic(uuid = "2a24", read, value = *b"Toucan")]
    model: [u8; 6],
}

async fn hid_host(stack: &Host<'_>, bonds: &Bonds) {
    let mut peripheral = stack.peripheral();
    let server = HidServer::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: "kb.wlt Toucan",
        appearance: &appearance::human_interface_device::KEYBOARD,
    }))
    .unwrap();
    let mut advertising = [0; 31];
    let length = AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::CompleteServiceUuids16(&[[0x12, 0x18]]),
            AdStructure::CompleteLocalName(b"kb.wlt Toucan"),
        ],
        &mut advertising,
    )
    .unwrap();
    loop {
        set_ble_ready(false);
        REPORTS.clear();
        let advertisement = peripheral
            .advertise(
                &Default::default(),
                Advertisement::ConnectableScannableUndirected {
                    adv_data: &advertising[..length],
                    scan_data: &[],
                },
            )
            .await;
        let Ok(advertisement) = advertisement else {
            Timer::after_millis(100).await;
            continue;
        };
        let Ok(connection) = advertisement.accept().await else {
            Timer::after_millis(100).await;
            continue;
        };
        if connection.set_bondable(true).is_err() {
            continue;
        }
        let Ok(connection) = connection.with_attribute_server(&server) else {
            continue;
        };
        connection.set(&server.hid.protocol_mode, &1).unwrap();
        connection.set(&server.hid.control_point, &1).unwrap();
        let send = async {
            loop {
                let boot = connection.get(&server.hid.protocol_mode).unwrap_or(1) == 0;
                let characteristic = if boot {
                    server.hid.boot_input
                } else {
                    server.hid.input_report
                };
                let ready = connection
                    .raw()
                    .security_level()
                    .map(|level| level.encrypted())
                    .unwrap_or(false)
                    && connection.get(&server.hid.control_point).unwrap_or(1) != 0
                    && characteristic.should_notify(&connection);
                set_ble_ready(ready);
                if !ready {
                    REPORTS.clear();
                    Timer::after_millis(10).await;
                    continue;
                }
                if let Ok((generation, report)) = REPORTS.try_receive() {
                    if transport().accepts(generation)
                        && characteristic
                            .notify(&connection, &report, true)
                            .await
                            .is_err()
                    {
                        return;
                    }
                }
                Timer::after_millis(1).await;
            }
        };
        let _ = select(
            gatt_events(
                &server.server,
                crate::ble_bonds::HID_SCHEMA,
                &connection,
                bonds,
                Some([
                    server.hid.protocol_mode.handle,
                    server.hid.control_point.handle,
                ]),
            ),
            send,
        )
        .await;
    }
}

async fn gatt_events<
    M: embassy_sync::blocking_mutex::raw::RawMutex,
    const AT: usize,
    const CN: usize,
>(
    server: &AttributeServer<'_, M, DefaultPacketPool, AT, CN>,
    schema: u32,
    connection: &GattConnection<'_, '_, DefaultPacketPool>,
    bonds: &Bonds,
    reset_handles: Option<[u16; 2]>,
) {
    loop {
        match connection.next().await {
            GattConnectionEvent::Disconnected { .. } => return,
            GattConnectionEvent::PairingComplete {
                bond: Some(bond), ..
            } => {
                let mut bonds = bonds.lock().await;
                bonds.remember(&bond).await;
                bonds
                    .remember_client_table(server, connection.raw(), schema)
                    .await;
            }
            GattConnectionEvent::Encrypted { security_level, .. } if security_level.encrypted() => {
                bonds
                    .lock()
                    .await
                    .restore_client_table(server, connection.raw(), schema);
            }
            GattConnectionEvent::Gatt { event } => {
                let changed = matches!(&event, GattEvent::Write(_));
                if let GattEvent::Write(ref write) = event {
                    if reset_handles.is_some_and(|handles| handles.contains(&write.handle())) {
                        BLE_SESSION.lock(|state| state.set(state.get().reset()));
                        REPORTS.clear();
                    }
                }
                if let Ok(reply) = event.accept() {
                    reply.send().await;
                }
                if changed {
                    bonds
                        .lock()
                        .await
                        .remember_client_table(server, connection.raw(), schema)
                        .await;
                }
            }
            _ => {}
        }
    }
}

async fn process_keys() {
    let mut component = KeyboardComponent::new(include_bytes!(env!("KB_KEYBOARD_WASM"))).ok();
    let mut state = Bounded::<u8, 64>::new(0);
    let mut reports = ReportState::<42>::new();
    let mut receivers = [Receiver::new(); 2];
    let mut previous = 0;
    let mut input_gate = InputGate::new();
    let mut wake_at = None;
    #[cfg(any(feature = "nice-view", feature = "prospector-display"))]
    let mut display_status = kb_display::Status {
        transport: if cfg!(feature = "toucan-dongle") {
            kb_display::Transport::Split
        } else {
            kb_display::Transport::Bluetooth
        },
        ..Default::default()
    };
    #[cfg(any(feature = "nice-view", feature = "prospector-display"))]
    let mut previous_display = display_status;
    loop {
        let mut invalidated = false;
        if let Either::First(event) = select(EVENTS.receive(), Timer::after_millis(5)).await {
            let now = Instant::now().as_millis() as u32;
            match event {
                LinkEvent::Open(slot, snapshot) => {
                    invalidated |= receivers[slot].connected();
                    receivers[slot].close();
                    let _ = receivers[slot].receive(snapshot, now);
                }
                LinkEvent::Snapshot(slot, snapshot) => {
                    invalidated |= receivers[slot].receive(snapshot, now).unwrap_or(false);
                }
                LinkEvent::Close(slot) => {
                    invalidated |= receivers[slot].connected();
                    receivers[slot].close();
                }
            }
        }
        let now = Instant::now().as_millis() as u32;
        for receiver in &mut receivers {
            invalidated |= receiver.expire(now);
        }
        let keys = receivers
            .iter()
            .fold(0, |keys, receiver| keys | receiver.keys());
        let session = transport();
        let ready = session.ready();
        let gate = input_gate.observe(session.generation(), ready, keys, invalidated);
        if gate.cancel {
            state = Bounded::new(0);
            reports.clear();
            wake_at = None;
            previous = 0;
            REPORTS.clear();
            if ready {
                REPORTS.send((session.generation(), [0; 8])).await;
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
                let output = component.process::<64, 42, 42>(state.as_slice(), &input)?;
                let wake = output.wake_at_ms;
                let output = component.transform_output::<64, 42>(
                    output.state.as_slice(),
                    output.actions.as_slice(),
                )?;
                let mut next_reports = reports;
                let mut pending = Bounded::<[u8; 8], 84>::new([0; 8]);
                for action in output.actions.as_slice() {
                    let update = next_reports
                        .apply(*action)
                        .map_err(|_| kb_component_runtime::Error::InvalidReturn)?;
                    pending
                        .push(update.first)
                        .map_err(|_| kb_component_runtime::Error::Capacity)?;
                    if let Some(second) = update.second {
                        pending
                            .push(second)
                            .map_err(|_| kb_component_runtime::Error::Capacity)?;
                    }
                }
                Ok::<_, kb_component_runtime::Error>((output.state, wake, next_reports, pending))
            })();
            match result {
                Ok((next_state, wake, next_reports, pending)) => {
                    state = next_state;
                    wake_at = wake;
                    reports = next_reports;
                    for report in pending.as_slice() {
                        REPORTS.send((session.generation(), *report)).await;
                        #[cfg(any(feature = "nice-view", feature = "prospector-display"))]
                        {
                            display_status.last_modifiers = report[0];
                            if report[2] != 0 {
                                display_status.last_usage = report[2];
                            }
                        }
                    }
                }
                Err(_) => {
                    component = None;
                    state = Bounded::new(0);
                    wake_at = None;
                    reports.clear();
                    REPORTS.clear();
                    REPORTS.send((session.generation(), [0; 8])).await;
                }
            }
            previous = keys;
        }
        COMPONENT_FAILED.store(component.is_none(), Ordering::Relaxed);
        #[cfg(any(feature = "nice-view", feature = "prospector-display"))]
        {
            display_status.connected = ready;
            display_status.pairing_unsaved = crate::ble_bonds::storage_failed();
            display_status.faulted = component.is_none();
            display_status.keyboard = Some(kb_display::KeyboardStatus {
                active_keys: keys.count_ones() as u8,
                layer: u8::from(keys & (1u64 << 41) != 0),
            });
            if display_status != previous_display {
                #[cfg(feature = "nice-view")]
                crate::nice_view::update(display_status);
                #[cfg(feature = "prospector-display")]
                crate::prospector::update(display_status);
                previous_display = display_status;
            }
        }
    }
}

fn transport() -> SessionGate {
    if cfg!(feature = "toucan-dongle") {
        crate::usb_hid::connection()
    } else {
        BLE_SESSION.lock(Cell::get)
    }
}

fn set_ble_ready(ready: bool) {
    BLE_SESSION.lock(|session| session.set(session.get().set_ready(ready)));
}

async fn run_usb(peripheral: Peri<'static, peripherals::USBD>) {
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
    let mut usb = builder.build();
    let send = async {
        let mut previous = [0; 8];
        let mut generation = u32::MAX;
        loop {
            writer.ready().await;
            let session = crate::usb_hid::connection();
            if generation != session.generation() {
                previous = [0; 8];
                generation = session.generation();
            }
            let (token, report) =
                match select(REPORTS.receive(), crate::usb_hid::idle_timeout()).await {
                    Either::First(report) => report,
                    Either::Second(_) => (generation, previous),
                };
            if crate::usb_hid::connection().accepts(token) && writer.write(&report).await.is_ok() {
                previous = report;
                crate::usb_hid::report_sent(report);
            }
        }
    };
    join(usb.run(), send).await;
}
