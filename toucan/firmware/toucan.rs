#[cfg(feature = "prospector-display")]
use crate::radio::DisplayIrqs;
use crate::radio::{Bonds, Host, address, device_id};
#[cfg(all(feature = "toucan-dongle", feature = "prospector-display"))]
use core::cell::Cell;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use embassy_executor::Spawner;
use embassy_futures::join::{join, join3};
#[cfg(feature = "nice-view")]
use embassy_futures::select::Either;
use embassy_futures::select::select;
#[cfg(not(feature = "toucan-dongle"))]
use embassy_nrf::gpio::Pull;
use embassy_nrf::gpio::{Input as GpioInput, Level, Output as GpioOutput, OutputDrive};
#[cfg(any(feature = "nice-view", feature = "prospector-display"))]
use embassy_nrf::interrupt::InterruptExt;
use embassy_nrf::interrupt::Priority;
#[cfg(feature = "nice-view")]
use embassy_nrf::peripherals;
#[cfg(all(feature = "toucan-dongle", feature = "prospector-display"))]
use embassy_sync::blocking_mutex::Mutex as BlockingMutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
#[cfg(feature = "toucan-dongle")]
use embassy_time::Duration;
use embassy_time::{Instant, Timer};
use kb_core::Debouncer;
#[cfg(feature = "nice-view")]
use kb_core::matrix_link::DisplayReceiver;
use kb_core::matrix_link::{DISPLAY_PACKET_LEN, KEYS_MASK, PACKET_LEN, Side, Snapshot};
#[cfg(any(
    feature = "nice-view",
    all(feature = "toucan-dongle", feature = "prospector-display")
))]
use kb_core::matrix_link::{DisplayFeedback, DisplayStatus};
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
const DISPLAY_UUID: Uuid = Uuid::new_long([
    0x45, 0x02, 0x12, 0x11, 0x4b, 0x57, 0xa7, 0x98, 0x6c, 0xaa, 0x03, 0xaf, 0x22, 0x00, 0x17, 0x7a,
]);
const DISPLAY_CAPABILITY: u8 = 1;

#[cfg(feature = "nice-view")]
static DISPLAY_INPUT: Signal<CriticalSectionRawMutex, (DisplayFeedback, u32)> = Signal::new();
#[cfg(all(feature = "toucan-dongle", feature = "prospector-display"))]
static DISPLAY_OUTPUT: BlockingMutex<CriticalSectionRawMutex, Cell<Option<(DisplayStatus, u32)>>> =
    BlockingMutex::new(Cell::new(None));

static COMPONENT_FAILED: AtomicBool = AtomicBool::new(false);
static KEYS: AtomicU32 = AtomicU32::new(0);
static KEY_CHANGED: Signal<CriticalSectionRawMutex, ()> = Signal::new();
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Satellite,
    Dongle,
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
        #[characteristic(uuid = DISPLAY_UUID, write_without_response, permissions(encrypted))]
        pub(super) display: [u8; DISPLAY_PACKET_LEN],
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
    let (central_count, peripheral_count) = match role {
        Role::Satellite => (0, 1),
        Role::Dongle => (2, 0),
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
    if let Some(matrix) = matrix.as_mut() {
        if matrix.read().await == KEYS_MASK as u64 {
            Timer::after_secs(2).await;
            if matrix.read().await == KEYS_MASK as u64 {
                bonds.forget().await;
            }
        }
    }
    #[cfg(feature = "toucan-dongle")]
    {
        let mut bytes = [0; 512];
        if let Ok(Some(n)) = bonds.load_config(&mut bytes) {
            crate::settings::initialize(Some(&bytes[..n]));
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
            Role::Dongle => {
                #[cfg(feature = "toucan-dongle")]
                join(
                    join(
                        crate::settings::run_saves(&bonds),
                        crate::settings::run_controls(&stack, &bonds),
                    ),
                    join3(
                        join(
                            receiver::peer_loop(&stack, &bonds, Side::Left),
                            receiver::peer_loop(&stack, &bonds, Side::Right),
                        ),
                        receiver::process_keys(),
                        receiver::run_usb(p.USBD),
                    ),
                )
                .await;
            }
        }
    };
    #[cfg(feature = "toucan-dongle")]
    let radio = runner.run_with_handler(&receiver::Discovery);
    #[cfg(not(feature = "toucan-dongle"))]
    let radio = runner.run();
    let keyboard = join3(radio, application, diagnostic);
    #[cfg(any(feature = "nice-view", feature = "prospector-display"))]
    let _ = join(keyboard, display).await;
    #[cfg(not(any(feature = "nice-view", feature = "prospector-display")))]
    let _ = keyboard.await;
}

async fn satellite(stack: &Host<'_>, bonds: &Bonds, side: Side, boot: u32) {
    let mut peripheral = stack.peripheral();
    let display_capable = cfg!(feature = "nice-view") && side == Side::Left;
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
                payload: &[
                    b'k',
                    b'b',
                    b'w',
                    2,
                    side as u8,
                    if display_capable {
                        DISPLAY_CAPABILITY
                    } else {
                        0
                    },
                ],
            },
        ],
        &mut advertising,
    )
    .unwrap();
    loop {
        #[cfg(feature = "nice-view")]
        if display_capable {
            show_remote_display(DisplayStatus::default());
        }
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
        let connected = select(
            gatt_events(
                &server.server,
                crate::ble_bonds::TOUCAN_LINK_SCHEMA,
                &connection,
                bonds,
                #[cfg(feature = "nice-view")]
                display_capable.then_some(server.keys.display.handle),
            ),
            notify,
        );
        #[cfg(feature = "nice-view")]
        if display_capable {
            DISPLAY_INPUT.reset();
            let _ = select(connected, run_remote_display()).await;
            continue;
        }
        let _ = connected.await;
    }
}

#[cfg(feature = "nice-view")]
fn show_remote_display(status: DisplayStatus) {
    crate::nice_view::update(kb_display::Status {
        transport: kb_display::Transport::Split,
        connected: status.connected,
        faulted: status.faulted,
        pairing_unsaved: status.pairing_unsaved || crate::ble_bonds::storage_failed(),
        last_usage: status.last_usage,
        last_modifiers: status.last_modifiers,
        keyboard: Some(kb_display::KeyboardStatus {
            active_keys: status.active_keys,
            layer: status.layer,
        }),
        ..Default::default()
    });
}

#[cfg(feature = "nice-view")]
async fn run_remote_display() {
    let mut receiver = DisplayReceiver::default();
    let mut previous = (DisplayStatus::default(), crate::ble_bonds::storage_failed());
    loop {
        if let Either::First((feedback, received_at)) =
            select(DISPLAY_INPUT.wait(), Timer::after_millis(100)).await
        {
            receiver.receive(feedback, received_at);
        }
        let status = receiver
            .current(Instant::now().as_millis() as u32)
            .unwrap_or_default();
        let next = (status, crate::ble_bonds::storage_failed());
        if next != previous {
            show_remote_display(status);
            previous = next;
        }
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
    #[cfg(feature = "nice-view")] display_handle: Option<u16>,
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
                #[cfg(feature = "nice-view")]
                let event = match event {
                    GattEvent::Write(write) if display_handle == Some(write.handle()) => {
                        let feedback = write.with_data(|offset, data| {
                            (offset == 0)
                                .then(|| DisplayFeedback::decode(data).ok())
                                .flatten()
                        });
                        if let Some(feedback) = feedback {
                            DISPLAY_INPUT.signal((feedback, Instant::now().as_millis() as u32));
                        }
                        if let Ok(reply) = write.accept_unprocessed() {
                            reply.send().await;
                        }
                        continue;
                    }
                    event => event,
                };
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

#[cfg(feature = "toucan-dongle")]
#[path = "receiver.rs"]
pub(crate) mod receiver;
