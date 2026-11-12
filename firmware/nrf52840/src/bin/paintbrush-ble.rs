#![no_std]
#![no_main]

use core::sync::atomic::{AtomicBool, Ordering};
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_futures::select::select;
use embassy_nrf::gpio::{Input, Level, Output, OutputDrive, Pull};
use embassy_nrf::{bind_interrupts, pac, peripherals, rng};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Instant, Timer};
use kb_component_runtime::{BrailleComponent, BrailleState};
use kb_core::{Debouncer, KeyStroke};
use nrf_sdc::{self as sdc, mpsl};
use panic_halt as _;
use static_cell::StaticCell;
use trouble_host::prelude::*;

bind_interrupts!(struct Irqs {
    RNG => rng::InterruptHandler<peripherals::RNG>;
    EGU0_SWI0 => mpsl::LowPrioInterruptHandler;
    CLOCK_POWER => mpsl::ClockInterruptHandler, embassy_nrf::usb::vbus_detect::InterruptHandler;
    RADIO => mpsl::HighPrioInterruptHandler;
    TIMER0 => mpsl::HighPrioInterruptHandler;
    RTC0 => mpsl::HighPrioInterruptHandler;
});

static READY: AtomicBool = AtomicBool::new(false);
static STROKES: Channel<CriticalSectionRawMutex, KeyStroke, 32> = Channel::new();
const DOTS: [u16; 8] = [64, 4, 2, 1, 8, 16, 32, 256];
const REPORT_DESCRIPTOR: [u8; 63] = [
    0x05, 0x01, 0x09, 0x06, 0xa1, 0x01, 0x05, 0x07, 0x19, 0xe0, 0x29, 0xe7, 0x15, 0x00, 0x25, 0x01,
    0x75, 0x01, 0x95, 0x08, 0x81, 0x02, 0x95, 0x01, 0x75, 0x08, 0x81, 0x01, 0x95, 0x05, 0x75, 0x01,
    0x05, 0x08, 0x19, 0x01, 0x29, 0x05, 0x91, 0x02, 0x95, 0x01, 0x75, 0x03, 0x91, 0x01, 0x95, 0x06,
    0x75, 0x08, 0x15, 0x00, 0x25, 0x65, 0x05, 0x07, 0x19, 0x00, 0x29, 0x65, 0x81, 0x00, 0xc0,
];

#[gatt_server(connections_max = 1, attribute_table_size = 40)]
struct Server {
    hid: HidService,
    _device: DeviceInformation,
}

#[gatt_service(uuid = "1812")]
struct HidService {
    #[characteristic(uuid = "2a4a", read, value = [0x11, 0x01, 0x00, 0x02], permissions(encrypted))]
    information: [u8; 4],
    #[characteristic(uuid = "2a4b", read, value = REPORT_DESCRIPTOR, permissions(encrypted))]
    report_map: [u8; 63],
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
    #[characteristic(uuid = "2a24", read, value = *b"Paintbrush")]
    model: [u8; 10],
}

#[embassy_executor::task]
async fn mpsl_task(mpsl: &'static mpsl::MultiprotocolServiceLayer<'static>) -> ! {
    mpsl.run().await
}

fn address() -> Address {
    let low = pac::FICR.deviceaddr(0).read().to_le_bytes();
    let high = pac::FICR.deviceaddr(1).read().to_le_bytes();
    Address::random([low[0], low[1], low[2], low[3], high[0], high[1] | 0xc0])
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    kb_nrf52840::init_heap();
    let mut config = kb_nrf52840::clock_config();
    config.time_interrupt_priority = embassy_nrf::interrupt::Priority::P3;
    let p = embassy_nrf::init(config);
    let mut indicator = Output::new(p.P0_15, Level::Low, OutputDrive::Standard);
    let inputs = [
        Input::new(p.P0_31, Pull::Up),
        Input::new(p.P0_29, Pull::Up),
        Input::new(p.P0_02, Pull::Up),
        Input::new(p.P1_15, Pull::Up),
        Input::new(p.P1_13, Pull::Up),
        Input::new(p.P1_11, Pull::Up),
        Input::new(p.P0_10, Pull::Up),
        Input::new(p.P0_09, Pull::Up),
    ];
    let mut component = BrailleComponent::new(include_bytes!(env!("KB_BRAILLE_WASM"))).unwrap();
    let mpsl_peripherals =
        mpsl::Peripherals::new(p.RTC0, p.TIMER0, p.TEMP, p.PPI_CH19, p.PPI_CH30, p.PPI_CH31);
    let lfclk = mpsl::raw::mpsl_clock_lfclk_cfg_t {
        source: mpsl::raw::MPSL_CLOCK_LF_SRC_XTAL as u8,
        rc_ctiv: 0,
        rc_temp_ctiv: 0,
        accuracy_ppm: 20,
        skip_wait_lfclk_started: false,
    };
    static MPSL: StaticCell<mpsl::MultiprotocolServiceLayer> = StaticCell::new();
    static TIMESLOTS: StaticCell<mpsl::SessionMem<1>> = StaticCell::new();
    let timeslots = TIMESLOTS.init(mpsl::SessionMem::new());
    let mpsl = MPSL.init(
        mpsl::MultiprotocolServiceLayer::with_timeslots(mpsl_peripherals, Irqs, lfclk, timeslots)
            .unwrap(),
    );
    spawner.spawn(mpsl_task(mpsl).unwrap());
    let mut bonds = kb_nrf52840::ble_bonds::BondStore::new(mpsl::Flash::take(mpsl, p.NVMC));
    if inputs.iter().all(|pin| pin.is_low()) {
        Timer::after_secs(2).await;
        if inputs.iter().all(|pin| pin.is_low()) {
            bonds.clear().await.unwrap();
        }
    }
    let saved_bond = bonds.load().unwrap();
    let sdc_peripherals = sdc::Peripherals::new(
        p.PPI_CH17, p.PPI_CH18, p.PPI_CH20, p.PPI_CH21, p.PPI_CH22, p.PPI_CH23, p.PPI_CH24,
        p.PPI_CH25, p.PPI_CH26, p.PPI_CH27, p.PPI_CH28, p.PPI_CH29,
    );
    let mut random = rng::Rng::new(p.RNG, Irqs);
    let mut sdc_memory = sdc::Mem::<8192>::new();
    let controller = sdc::Builder::new()
        .unwrap()
        .support_adv()
        .support_peripheral()
        .peripheral_count(1)
        .unwrap()
        .buffer_cfg(128, 128, 3, 3)
        .unwrap()
        .build(sdc_peripherals, &mut random, mpsl, &mut sdc_memory)
        .unwrap();
    let mut resources: HostResources<DefaultPacketPool, 1, 3> = HostResources::new();
    let stack = trouble_host::new(controller, &mut resources)
        .set_random_address(address())
        .set_io_capabilities(IoCapabilities::NoInputNoOutput)
        .build();
    if let Some(bond) = saved_bond {
        stack.add_bond_information(bond).unwrap();
    }
    let mut runner = stack.runner();
    let mut peripheral = stack.peripheral();
    let server = Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: "kb.wlt Paintbrush",
        appearance: &appearance::human_interface_device::KEYBOARD,
    }))
    .unwrap();
    let scan = async {
        let mut debounce = Debouncer::<8>::new(5);
        let mut state = BrailleState::default();
        let mut armed = false;
        loop {
            let raw = inputs
                .iter()
                .enumerate()
                .fold(0, |mask, (i, pin)| mask | (u64::from(pin.is_low()) << i));
            let (next, update) = debounce.sample(raw, Instant::now().as_millis() as u32);
            debounce = next;
            if !READY.load(Ordering::Relaxed) {
                state = BrailleState::default();
                armed = false;
            } else if update.stable == 0 {
                armed = true;
            }
            if armed && (update.pressed | update.released) != 0 {
                let mut dots = 0;
                for position in 0..8 {
                    if update.stable & (1 << position) != 0 {
                        let dot_index = if cfg!(feature = "paintbrush-left") {
                            position ^ 3
                        } else {
                            position
                        };
                        dots |= DOTS[dot_index];
                    }
                }
                let output = component.step(state, dots).unwrap();
                state = output.state;
                if output.usage != 0 {
                    let _ = STROKES.try_send(KeyStroke::new(output.usage, output.modifiers));
                }
            }
            indicator.set_level(if update.stable != 0 {
                Level::High
            } else {
                Level::Low
            });
            Timer::after_millis(1).await;
        }
    };
    let connections = async {
        let mut advertising = [0; 31];
        let len = AdStructure::encode_slice(
            &[
                AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
                AdStructure::CompleteServiceUuids16(&[[0x12, 0x18]]),
                AdStructure::CompleteLocalName(b"kb.wlt Paintbrush"),
            ],
            &mut advertising,
        )
        .unwrap();
        loop {
            READY.store(false, Ordering::Relaxed);
            STROKES.clear();
            let advertiser = peripheral
                .advertise(
                    &Default::default(),
                    Advertisement::ConnectableScannableUndirected {
                        adv_data: &advertising[..len],
                        scan_data: &[],
                    },
                )
                .await
                .unwrap();
            let connection = advertiser.accept().await.unwrap();
            connection.set_bondable(true).unwrap();
            let connection = connection.with_attribute_server(&server).unwrap();
            let _ = select(
                gatt_events(&connection, &mut bonds),
                send_reports(&server, &connection),
            )
            .await;
        }
    };
    let ble = async {
        let _ = join(runner.run(), connections).await;
    };
    join(ble, scan).await;
}

async fn gatt_events(
    connection: &GattConnection<'_, '_, DefaultPacketPool>,
    bonds: &mut kb_nrf52840::ble_bonds::BondStore<'_>,
) {
    loop {
        match connection.next().await {
            GattConnectionEvent::Disconnected { .. } => return,
            GattConnectionEvent::PairingComplete {
                bond: Some(bond), ..
            } => {
                bonds.save(&bond).await.unwrap();
            }
            GattConnectionEvent::Gatt { event } => {
                if let Ok(reply) = event.accept() {
                    reply.send().await;
                }
            }
            _ => {}
        }
    }
}

async fn send_reports(server: &Server<'_>, connection: &GattConnection<'_, '_, DefaultPacketPool>) {
    loop {
        let boot = connection.get(&server.hid.protocol_mode).unwrap_or(1) == 0;
        let report = if boot {
            server.hid.boot_input
        } else {
            server.hid.input_report
        };
        let ready = connection
            .raw()
            .security_level()
            .map(|level| level.encrypted())
            .unwrap_or(false)
            && report.should_notify(connection);
        READY.store(ready, Ordering::Relaxed);
        if !ready {
            STROKES.clear();
            Timer::after_millis(10).await;
            continue;
        }
        if let Ok(stroke) = STROKES.try_receive() {
            if report
                .notify(connection, &stroke.report(), true)
                .await
                .is_err()
            {
                return;
            }
            if report.notify(connection, &[0; 8], true).await.is_err() {
                return;
            }
        }
        Timer::after_millis(1).await;
    }
}
