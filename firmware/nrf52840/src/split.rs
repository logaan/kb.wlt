use crate::ble_bonds::BondStore;
use core::sync::atomic::{AtomicU8, Ordering};
use embassy_executor::Spawner;
use embassy_futures::join::{join, join3};
use embassy_futures::select::{Either, select};
use embassy_nrf::gpio::{Input, Level, Output, OutputDrive, Pull};
use embassy_nrf::interrupt::{InterruptExt, Priority};
use embassy_nrf::mode::Async;
use embassy_nrf::usb::{Driver, vbus_detect::HardwareVbusDetect};
use embassy_nrf::{Peri, bind_interrupts, pac, peripherals, rng, usb};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Instant, Timer, with_timeout};
use embassy_usb::class::hid::{
    Config as HidConfig, HidBootProtocol, HidSubclass, HidWriter, State,
};
use embassy_usb::{Builder, Config};
use kb_component_runtime::{BrailleComponent, BrailleState};
use kb_core::hid::KEYBOARD_REPORT_DESCRIPTOR as REPORT_DESCRIPTOR;
use kb_core::split::{Layout, PACKET_LEN, Receiver, Session, Snapshot};
use kb_core::{Debouncer, KeyStroke};
use nrf_sdc::{self as sdc, mpsl};
use static_cell::StaticCell;
use trouble_host::prelude::*;

bind_interrupts!(pub(crate) struct Irqs {
    RNG => rng::InterruptHandler<peripherals::RNG>;
    EGU0_SWI0 => mpsl::LowPrioInterruptHandler;
    CLOCK_POWER => mpsl::ClockInterruptHandler, usb::vbus_detect::InterruptHandler;
    RADIO => mpsl::HighPrioInterruptHandler;
    TIMER0 => mpsl::HighPrioInterruptHandler;
    RTC0 => mpsl::HighPrioInterruptHandler;
    USBD => usb::InterruptHandler<peripherals::USBD>;
});

#[cfg(feature = "prospector-display")]
bind_interrupts!(pub(crate) struct DisplayIrqs {
    SPIM3 => embassy_nrf::spim::InterruptHandler<peripherals::SPI3>;
});

const SERVICE_BYTES: [u8; 16] = [
    0x43, 0x02, 0x12, 0x11, 0x4b, 0x57, 0xa7, 0x98, 0x6c, 0xaa, 0x03, 0xaf, 0x11, 0x00, 0x17, 0x7a,
];
const SERVICE_UUID: Uuid = Uuid::new_long(SERVICE_BYTES);
const SNAPSHOT_UUID: Uuid = Uuid::new_long([
    0x44, 0x02, 0x12, 0x11, 0x4b, 0x57, 0xa7, 0x98, 0x6c, 0xaa, 0x03, 0xaf, 0x11, 0x00, 0x17, 0x7a,
]);

static KEYS: AtomicU8 = AtomicU8::new(0);
static KEY_CHANGED: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static STROKES: Channel<CriticalSectionRawMutex, (u32, KeyStroke), 32> = Channel::new();
static EVENTS: Channel<CriticalSectionRawMutex, LinkEvent, 64> = Channel::new();
static ADVERTISEMENTS: Channel<CriticalSectionRawMutex, Discovered, 16> = Channel::new();
static CONNECT_LOCK: Mutex<CriticalSectionRawMutex, ()> = Mutex::new(());

pub(crate) type ControllerType = sdc::SoftdeviceController<'static>;
pub(crate) type Host<'d> = Stack<'d, ControllerType, DefaultPacketPool>;
pub(crate) type Bonds = Mutex<CriticalSectionRawMutex, BondStore<'static>>;

#[derive(Clone, Copy)]
struct Discovered {
    address: Address,
    layout: Layout,
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
            if let Some(layout) = advertised_layout(report.data) {
                let _ = ADVERTISEMENTS.try_send(Discovered {
                    address: Address::new(report.addr_kind, report.addr),
                    layout,
                });
            }
        }
    }
}

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
                0 => Some(Layout::Single),
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

#[embassy_executor::task]
async fn mpsl_task(mpsl: &'static mpsl::MultiprotocolServiceLayer<'static>) -> ! {
    mpsl.run().await
}

pub(crate) fn device_id() -> u64 {
    pac::FICR.deviceid(0).read() as u64 | ((pac::FICR.deviceid(1).read() as u64) << 32)
}

pub(crate) fn address() -> Address {
    let low = pac::FICR.deviceaddr(0).read().to_le_bytes();
    let high = pac::FICR.deviceaddr(1).read().to_le_bytes();
    Address::random([low[0], low[1], low[2], low[3], high[0], high[1] | 0xc0])
}

pub(crate) fn radio(
    spawner: Spawner,
    mpsl_peripherals: mpsl::Peripherals<'static>,
    sdc_peripherals: sdc::Peripherals<'static>,
    random: Peri<'static, peripherals::RNG>,
    flash: Peri<'static, peripherals::NVMC>,
    central_count: u8,
    peripheral_count: u8,
) -> (ControllerType, BondStore<'static>, u32) {
    embassy_nrf::interrupt::RNG.set_priority(Priority::P3);
    embassy_nrf::interrupt::USBD.set_priority(Priority::P3);
    static MPSL: StaticCell<mpsl::MultiprotocolServiceLayer> = StaticCell::new();
    static TIMESLOTS: StaticCell<mpsl::SessionMem<1>> = StaticCell::new();
    let mpsl = MPSL.init(
        mpsl::MultiprotocolServiceLayer::with_timeslots(
            mpsl_peripherals,
            Irqs,
            mpsl::raw::mpsl_clock_lfclk_cfg_t {
                source: mpsl::raw::MPSL_CLOCK_LF_SRC_XTAL as u8,
                rc_ctiv: 0,
                rc_temp_ctiv: 0,
                accuracy_ppm: 20,
                skip_wait_lfclk_started: false,
            },
            TIMESLOTS.init(mpsl::SessionMem::new()),
        )
        .unwrap(),
    );
    spawner.spawn(mpsl_task(mpsl).unwrap());
    static RNG: StaticCell<rng::Rng<'static, Async>> = StaticCell::new();
    let rng = RNG.init(rng::Rng::new(random, Irqs));
    let mut boot = [0; 4];
    rng.blocking_fill_bytes(&mut boot);
    static MEMORY: StaticCell<sdc::Mem<16384>> = StaticCell::new();
    let mut builder = sdc::Builder::new().unwrap();
    if central_count != 0 {
        builder = builder.support_scan().support_central();
    }
    if peripheral_count != 0 {
        builder = builder.support_adv().support_peripheral();
    }
    builder = builder
        .central_count(central_count)
        .unwrap()
        .peripheral_count(peripheral_count)
        .unwrap();
    let controller = builder
        .buffer_cfg(128, 128, 4, 4)
        .unwrap()
        .build(sdc_peripherals, rng, mpsl, MEMORY.init(sdc::Mem::new()))
        .unwrap();
    (
        controller,
        BondStore::new(mpsl::Flash::take(mpsl, flash)),
        u32::from_le_bytes(boot),
    )
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
        let keys = if cfg!(feature = "paintbrush-left") {
            ((update.stable as u8 & 0x88) >> 3)
                | ((update.stable as u8 & 0x44) >> 1)
                | ((update.stable as u8 & 0x22) << 1)
                | ((update.stable as u8 & 0x11) << 3)
        } else {
            update.stable as u8
        };
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
    } else if cfg!(feature = "satellite-right") {
        Layout::Right
    } else {
        Layout::Single
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
                            bonds.lock().await.remember(&bond).await;
                        }
                        GattConnectionEvent::Gatt { event } => {
                            if let Ok(reply) = event.accept() {
                                reply.send().await;
                            }
                        }
                        _ => {}
                    }
                }
            };
            let _ = select(events, notify).await;
        }
    };
    join3(runner.run(), connections, scan(inputs, indicator)).await;
}

pub async fn central(spawner: Spawner, local_keyboard: bool) {
    crate::init_heap();
    let mut config = crate::clock_config();
    config.time_interrupt_priority = Priority::P3;
    let p = embassy_nrf::init(config);
    #[cfg(not(feature = "prospector-display"))]
    let local = if local_keyboard {
        Some((
            inputs(
                p.P0_31, p.P0_29, p.P0_02, p.P1_15, p.P1_13, p.P1_11, p.P0_10, p.P0_09,
            ),
            Output::new(p.P0_15, Level::Low, OutputDrive::Standard),
        ))
    } else {
        None
    };
    #[cfg(feature = "prospector-display")]
    let local: Option<([Input<'static>; 8], Output<'static>)> = None;
    #[cfg(feature = "prospector-display")]
    let display = {
        assert!(!local_keyboard);
        embassy_nrf::interrupt::SPIM3.set_priority(Priority::P3);
        let mut spi_config = embassy_nrf::spim::Config::default();
        spi_config.frequency = embassy_nrf::spim::Frequency::M16;
        let spi =
            embassy_nrf::spim::Spim::new_txonly(p.SPI3, DisplayIrqs, p.P1_13, p.P1_15, spi_config);
        crate::prospector::run(
            spi,
            Output::new(p.P1_14, Level::High, OutputDrive::Standard),
            Output::new(p.P1_12, Level::High, OutputDrive::Standard),
            Output::new(p.P0_29, Level::High, OutputDrive::Standard),
            Output::new(p.P1_11, Level::High, OutputDrive::Standard),
        )
    };
    let (controller, mut bonds, boot) = radio(
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
    if let Some((ref inputs, _)) = local {
        if inputs.iter().all(Input::is_low) {
            Timer::after_secs(2).await;
            if inputs.iter().all(Input::is_low) {
                bonds.forget().await;
            }
        }
    }
    let saved = bonds.load_all().unwrap();
    let mut resources: HostResources<DefaultPacketPool, 2, 6> = HostResources::new();
    let stack = trouble_host::new(controller, &mut resources)
        .set_random_address(address())
        .build();
    for bond in saved.into_iter().flatten() {
        stack.add_bond_information(bond).unwrap();
    }
    let bonds = Bonds::new(bonds);
    let mut runner = stack.runner();
    let links = async {
        if local_keyboard {
            join(
                peer_loop(&stack, &bonds, 1, Layout::Right),
                local_snapshots(boot),
            )
            .await;
        } else if cfg!(feature = "split-pair") {
            join(
                peer_loop(&stack, &bonds, 0, Layout::Left),
                peer_loop(&stack, &bonds, 1, Layout::Right),
            )
            .await;
        } else {
            peer_loop(&stack, &bonds, 0, Layout::Single).await;
        }
    };
    let scanner = async {
        if let Some((inputs, indicator)) = local {
            scan(inputs, indicator).await;
        } else {
            core::future::pending::<()>().await;
        }
    };
    let application = join3(process_keys(), run_usb(p.USBD), scanner);
    let keyboard = join3(runner.run_with_handler(&Discovery), links, application);
    #[cfg(feature = "prospector-display")]
    let _ = join(keyboard, display).await;
    #[cfg(not(feature = "prospector-display"))]
    let _ = keyboard.await;
}

async fn local_snapshots(boot: u32) {
    let mut sequence = 0;
    let mut first = true;
    loop {
        let snapshot = Snapshot {
            node: device_id(),
            boot,
            sequence,
            layout: Layout::Left,
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

async fn peer_loop(stack: &Host<'_>, bonds: &Bonds, slot: usize, layout: Layout) {
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
                Ok(Ok(connection)) => connection,
                _ => continue,
            }
        };
        if authenticate(&connection, stack, bonds).await {
            receive_snapshots(stack, bonds, &connection, slot, layout).await;
        }
        connection.disconnect();
        EVENTS.send(LinkEvent::Close(slot)).await;
        Timer::after_millis(100).await;
    }
}

pub(crate) async fn authenticate(
    connection: &Connection<'_, DefaultPacketPool>,
    stack: &Host<'_>,
    bonds: &Bonds,
) -> bool {
    if connection.set_bondable(true).is_err() || connection.request_security().is_err() {
        return false;
    }
    let wait = async {
        loop {
            match connection.next().await {
                ConnectionEvent::Encrypted { security_level, .. } => {
                    return security_level.encrypted();
                }
                ConnectionEvent::PairingComplete {
                    security_level,
                    bond,
                } => {
                    if let Some(bond) = bond {
                        bonds.lock().await.remember(&bond).await;
                    }
                    return security_level.encrypted();
                }
                ConnectionEvent::PairingFailed(_) | ConnectionEvent::Disconnected { .. } => {
                    return false;
                }
                ConnectionEvent::RequestConnectionParams(request) => {
                    let _ = request.accept(None, stack).await;
                }
                _ => {}
            }
        }
    };
    with_timeout(Duration::from_secs(30), wait)
        .await
        .unwrap_or(false)
}

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

async fn process_keys() {
    let mut component = kb_core::transport::FaultLatch::new(BrailleComponent::new(include_bytes!(
        env!("KB_BRAILLE_WASM")
    )));
    let mut fault_generation = None;
    let mut state = BrailleState::default();
    #[cfg(feature = "prospector-display")]
    let catalog = {
        let mut catalog = kb_core::reference::Catalog::new();
        let mut failed = false;
        for cell in 1..=511 {
            let result = component.call(|component| {
                let packed = component.map_cell(cell)?;
                catalog
                    .append(cell, packed)
                    .map_err(|_| kb_component_runtime::Error::InvalidReturn)
            });
            if result.is_err() {
                failed = true;
                break;
            }
            embassy_futures::yield_now().await;
        }
        if failed { None } else { Some(catalog) }
    };
    #[cfg(feature = "prospector-display")]
    let mut reference = kb_core::reference::State::new();
    #[cfg(feature = "prospector-display")]
    let mut cursor = kb_core::reference::Cursor::default();
    #[cfg(feature = "prospector-display")]
    let mut reference_page = None;
    let mut receiver = Receiver::<2>::new(500);
    let mut sessions: [Option<Session>; 2] = [None; 2];
    let mut armed = false;
    let mut generation = 0;
    #[cfg(feature = "prospector-display")]
    let mut display_status = kb_display::Status {
        transport: kb_display::Transport::Split,
        ..Default::default()
    };
    loop {
        let now = Instant::now().as_millis() as u32;
        let (next, mut update) = receiver.expire(now);
        receiver = next;
        if update.cancel_chord {
            state = BrailleState::default();
            armed = false;
        }
        if let Either::First(event) = select(EVENTS.receive(), Timer::after_millis(5)).await {
            let now = Instant::now().as_millis() as u32;
            let result = match event {
                LinkEvent::Open(slot, snapshot) => {
                    let (next, session, opened) = receiver
                        .connect(slot, snapshot.node, snapshot.layout, now)
                        .unwrap();
                    receiver = next;
                    sessions[slot] = Some(session);
                    update.cancel_chord |= opened.cancel_chord;
                    if opened.cancel_chord {
                        state = BrailleState::default();
                        armed = false;
                    }
                    receiver.receive(session, snapshot, now)
                }
                LinkEvent::Snapshot(slot, snapshot) => {
                    if !receiver.is_connected(slot) {
                        let (next, session, opened) = receiver
                            .connect(slot, snapshot.node, snapshot.layout, now)
                            .unwrap();
                        receiver = next;
                        update.cancel_chord |= opened.cancel_chord;
                        sessions[slot] = Some(session);
                    }
                    match sessions[slot] {
                        Some(session) => receiver.receive(session, snapshot, now),
                        None => continue,
                    }
                }
                LinkEvent::Close(slot) => match sessions[slot].take() {
                    Some(session) => receiver.disconnect(session),
                    None => continue,
                },
            };
            if let Ok((next, received)) = result {
                update.cancel_chord |= received.cancel_chord;
                update.keys = received.keys;
                update.pressed |= received.pressed;
                update.released |= received.released;
                receiver = next;
            }
        }
        let session = crate::usb_hid::connection();
        if !session.ready() || generation != session.generation() || update.cancel_chord {
            STROKES.clear();
            fault_generation = None;
            generation = session.generation();
            #[cfg(feature = "prospector-display")]
            {
                reference = reference.cancel(update.keys).state;
            }
            state = BrailleState::default();
            armed = false;
        } else if update.keys == 0 && !component.is_faulted() {
            armed = true;
        }
        #[cfg(feature = "prospector-display")]
        let forward = if let Some(catalog) = catalog.as_ref().filter(|_| armed) {
            let transition = reference.step(update.keys, Instant::now().as_millis() as u32);
            reference = transition.state;
            cursor = cursor.apply(transition.action, catalog);
            if transition.reset_braille {
                state = BrailleState::default();
                STROKES.clear();
            }
            transition.forward
        } else {
            false
        };
        #[cfg(not(feature = "prospector-display"))]
        let forward = true;
        if armed && forward && update.keys != state.held {
            if let Ok(output) = component.call(|component| component.step(state, update.keys)) {
                state = output.state;
                if output.usage != 0 {
                    let _ = STROKES
                        .try_send((generation, KeyStroke::new(output.usage, output.modifiers)));
                    #[cfg(feature = "prospector-display")]
                    {
                        display_status.last_usage = output.usage;
                        display_status.last_modifiers = output.modifiers;
                    }
                }
            }
        }
        if component.is_faulted() {
            state = BrailleState::default();
            armed = false;
            #[cfg(feature = "prospector-display")]
            {
                reference = reference.cancel(update.keys).state;
            }
            if session.ready() && fault_generation != Some(generation) {
                STROKES.clear();
                if STROKES.try_send((generation, KeyStroke::new(0, 0))).is_ok() {
                    fault_generation = Some(generation);
                }
            }
        }
        #[cfg(feature = "prospector-display")]
        {
            let connected =
                session.ready() && (receiver.is_connected(0) || receiver.is_connected(1));
            let pairing_unsaved = crate::ble_bonds::storage_failed();
            let changed = display_status.pairing_unsaved != pairing_unsaved
                || display_status.faulted != component.is_faulted()
                || display_status.connected != connected
                || display_status.held != state.held
                || display_status.chord != state.chord
                || update.pressed | update.released != 0;
            display_status.connected = connected;
            display_status.faulted = component.is_faulted();
            display_status.pairing_unsaved = pairing_unsaved;
            display_status.held = state.held;
            display_status.chord = state.chord;
            if matches!(
                reference.mode,
                kb_core::reference::Mode::Namespace
                    | kb_core::reference::Mode::Reference
                    | kb_core::reference::Mode::Find
            ) && let Some(catalog) = catalog.as_ref()
            {
                let page = cursor.page(catalog, reference.mode);
                if reference_page != Some(page) {
                    reference_page = Some(page);
                    crate::prospector::show_reference(page);
                }
            } else if changed || reference_page.take().is_some() {
                crate::prospector::update(display_status);
            }
        }
    }
}

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
    let mut usb = builder.build();
    let reports = async {
        loop {
            writer.ready().await;
            if writer.write(&[0; 8]).await.is_ok() {
                crate::usb_hid::report_sent([0; 8]);
            }
            if let Either::First((generation, stroke)) =
                select(STROKES.receive(), crate::usb_hid::idle_timeout()).await
            {
                if !crate::usb_hid::connection().accepts(generation) {
                    continue;
                }
                if writer.write(&stroke.report()).await.is_ok() {
                    crate::usb_hid::report_sent(stroke.report());
                    if writer.write(&[0; 8]).await.is_ok() {
                        crate::usb_hid::report_sent([0; 8]);
                    }
                }
            }
        }
    };
    join(usb.run(), reports).await;
}
