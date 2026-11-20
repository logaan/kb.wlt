#[cfg(feature = "prospector-display")]
use crate::radio::DisplayIrqs;
use crate::radio::{Bonds, address};
#[cfg(feature = "toucan-right")]
use crate::radio::{Host, device_id};
#[cfg(not(feature = "toucan-dongle"))]
use core::sync::atomic::AtomicU32;
use core::sync::atomic::{AtomicBool, Ordering};
use embassy_executor::Spawner;
use embassy_futures::join::{join, join3};
#[cfg(not(feature = "toucan-dongle"))]
use embassy_futures::select::select;
#[cfg(not(feature = "toucan-dongle"))]
use embassy_nrf::gpio::{Input as GpioInput, Pull};
use embassy_nrf::gpio::{Level, Output as GpioOutput, OutputDrive};
#[cfg(any(feature = "toucan-display", feature = "prospector-display"))]
use embassy_nrf::interrupt::InterruptExt;
use embassy_nrf::interrupt::Priority;
#[cfg(feature = "toucan-display")]
use embassy_nrf::peripherals;
#[cfg(not(feature = "toucan-dongle"))]
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::{Instant, Timer};
#[cfg(not(feature = "toucan-dongle"))]
use kb_core::Debouncer;
#[cfg(not(feature = "toucan-dongle"))]
use kb_core::matrix_link::PACKET_LEN;
#[cfg(feature = "toucan-right")]
use kb_core::matrix_link::Snapshot;
#[cfg(not(feature = "toucan-dongle"))]
use kb_core::matrix_link::{KEYS_MASK, Side};
use nrf_sdc::{self as sdc, mpsl};
use trouble_host::prelude::*;

#[cfg(feature = "toucan-display")]
embassy_nrf::bind_interrupts!(struct DisplayIrqs {
    SPIM3 => embassy_nrf::spim::InterruptHandler<peripherals::SPI3>;
});

#[cfg(not(feature = "toucan-dongle"))]
const SERVICE_BYTES: [u8; 16] = [
    0x43, 0x02, 0x12, 0x11, 0x4b, 0x57, 0xa7, 0x98, 0x6c, 0xaa, 0x03, 0xaf, 0x22, 0x00, 0x17, 0x7a,
];
#[cfg(not(feature = "toucan-dongle"))]
const SERVICE_UUID: Uuid = Uuid::new_long(SERVICE_BYTES);
#[cfg(not(feature = "toucan-dongle"))]
const SNAPSHOT_UUID: Uuid = Uuid::new_long([
    0x44, 0x02, 0x12, 0x11, 0x4b, 0x57, 0xa7, 0x98, 0x6c, 0xaa, 0x03, 0xaf, 0x22, 0x00, 0x17, 0x7a,
]);
static COMPONENT_FAILED: AtomicBool = AtomicBool::new(false);
#[cfg(not(feature = "toucan-dongle"))]
static KEYS: AtomicU32 = AtomicU32::new(0);
#[cfg(not(feature = "toucan-dongle"))]
static KEY_CHANGED: Signal<CriticalSectionRawMutex, ()> = Signal::new();
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Satellite,
    Dongle,
}

#[cfg(feature = "toucan-right")]
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
#[cfg(feature = "toucan-right")]
use satellite_gatt::SatelliteServer;

#[cfg(not(feature = "toucan-dongle"))]
struct Matrix {
    rows: [GpioInput<'static>; 4],
    columns: [GpioOutput<'static>; 6],
    side: Side,
}

#[cfg(not(feature = "toucan-dongle"))]
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
    #[cfg(not(feature = "toucan-dongle"))]
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
        Matrix {
            rows: [
                GpioInput::new(p.P0_19, Pull::Down),
                GpioInput::new(p.P0_28, Pull::Down),
                GpioInput::new(p.P1_01, Pull::Down),
                GpioInput::new(p.P0_29, Pull::Down),
            ],
            columns,
            side,
        }
    };
    #[cfg(feature = "toucan-display")]
    let display = {
        embassy_nrf::interrupt::SPIM3.set_priority(Priority::P3);
        let mut config = embassy_nrf::spim::Config::default();
        config.frequency = embassy_nrf::spim::Frequency::M1;
        config.bit_order = embassy_nrf::spim::BitOrder::LsbFirst;
        let spi =
            embassy_nrf::spim::Spim::new_txonly(p.SPI3, DisplayIrqs, p.P1_13, p.P1_15, config);
        crate::toucan_display::run(
            spi,
            GpioOutput::new(p.P0_03, Level::Low, OutputDrive::Standard),
        )
    };
    #[cfg(feature = "prospector-display")]
    let display = {
        embassy_nrf::interrupt::SPI2.set_priority(Priority::P3);
        let mut config = embassy_nrf::spim::Config::default();
        config.frequency = embassy_nrf::spim::Frequency::M4;
        let spi =
            embassy_nrf::spim::Spim::new_txonly(p.SPI2, DisplayIrqs, p.P1_13, p.P1_15, config);
        crate::prospector::run(
            spi,
            GpioOutput::new(p.P1_14, Level::High, OutputDrive::Standard),
            GpioOutput::new(p.P1_12, Level::High, OutputDrive::Standard),
            GpioOutput::new(p.P0_29, Level::High, OutputDrive::Standard),
            GpioOutput::new(p.P1_11, Level::Low, OutputDrive::Standard),
        )
    };
    let (central_count, peripheral_count) = if cfg!(feature = "toucan-display") {
        (1, 1)
    } else if role == Role::Dongle {
        (1, 0)
    } else {
        (0, 1)
    };
    let (controller, mut bonds, boot) = crate::radio::radio(
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
    #[cfg(not(feature = "toucan-dongle"))]
    if matrix.read().await == u64::from(KEYS_MASK) {
        Timer::after_secs(2).await;
        if matrix.read().await == u64::from(KEYS_MASK) {
            bonds.forget().await;
        }
    }
    #[cfg(any(feature = "toucan-display", feature = "toucan-dongle"))]
    {
        let mut bytes = [0; 512];
        if let Ok(Some(n)) = bonds.load_config(&mut bytes) {
            crate::settings::initialize(Some(&bytes[..n]));
        }
    }
    #[cfg(feature = "toucan-display")]
    route::initialize();
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
    #[cfg(not(feature = "toucan-dongle"))]
    let battery = async {
        let _divider = GpioOutput::new(p.P0_14, Level::Low, OutputDrive::Standard0Disconnect1);
        crate::battery::monitor(
            p.SAADC,
            embassy_nrf::saadc::ChannelConfig::single_ended(p.P0_31),
            1510,
            510,
        )
        .await;
    };
    #[cfg(not(feature = "toucan-dongle"))]
    let scanner = join(matrix.scan(), battery);
    #[cfg(feature = "toucan-display")]
    {
        let _ = boot;
        spawner.spawn(application::process_keys().unwrap());
    }
    #[cfg(feature = "toucan-display")]
    let application = join(
        scanner,
        join(
            join(
                crate::settings::run_saves(&bonds),
                crate::settings::run_controls(&stack, &bonds),
            ),
            join(right_link::run(&stack, &bonds), async {
                loop {
                    while !crate::settings::bluetooth_enabled() {
                        Timer::after_millis(100).await;
                    }
                    let direct = route::direct();
                    let _ = select(
                        async {
                            if direct {
                                ble_hid::advertise(&stack, &bonds).await;
                            } else {
                                relay::advertise(&stack, &bonds).await;
                            }
                        },
                        route::changed(direct),
                    )
                    .await;
                    route::set_ready(false);
                }
            }),
        ),
    );
    #[cfg(feature = "toucan-right")]
    let application = join(satellite(&stack, &bonds, side, boot), scanner);
    #[cfg(feature = "toucan-dongle")]
    let application = {
        let _ = boot;
        join(relay::connect(&stack, &bonds), receiver::run_usb(p.USBD))
    };

    #[cfg(feature = "toucan-display")]
    let radio = runner.run_with_handler(&right_link::Discovery);
    #[cfg(feature = "toucan-dongle")]
    let radio = runner.run_with_handler(&relay::Discovery);
    #[cfg(feature = "toucan-right")]
    let radio = runner.run();
    let keyboard = join3(radio, application, diagnostic);
    #[cfg(any(feature = "toucan-display", feature = "prospector-display"))]
    let _ = join(keyboard, display).await;
    #[cfg(not(any(feature = "toucan-display", feature = "prospector-display")))]
    let _ = keyboard.await;
}

#[cfg(feature = "toucan-right")]
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
                        return;
                    }
                    sequence = sequence.wrapping_add(1);
                }
                let _ = select(KEY_CHANGED.wait(), Timer::after_millis(100)).await;
            }
        };
        let connected = select(
            gatt_events(
                &server.server,
                crate::ble_bonds::TOUCAN_LINK_SCHEMA,
                &connection,
                bonds,
            ),
            notify,
        );
        let _ = connected.await;
    }
}

#[cfg(feature = "toucan-right")]
async fn gatt_events<
    M: embassy_sync::blocking_mutex::raw::RawMutex,
    const AT: usize,
    const CN: usize,
>(
    server: &AttributeServer<'_, M, DefaultPacketPool, AT, CN>,
    schema: u32,
    connection: &GattConnection<'_, '_, DefaultPacketPool>,
    bonds: &Bonds,
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

#[cfg(feature = "toucan-display")]
#[path = "application.rs"]
mod application;
#[cfg(feature = "toucan-display")]
#[path = "ble_hid.rs"]
mod ble_hid;
#[cfg(feature = "toucan-dongle")]
#[path = "receiver.rs"]
pub(crate) mod receiver;
#[cfg(any(feature = "toucan-display", feature = "toucan-dongle"))]
#[path = "relay.rs"]
mod relay;
#[cfg(feature = "toucan-display")]
#[path = "right_link.rs"]
mod right_link;
#[cfg(feature = "toucan-display")]
#[path = "route.rs"]
mod route;
