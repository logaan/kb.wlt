#![no_std]
#![no_main]

use core::cell::Cell;
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_futures::select::{Either, select};
use embassy_nrf::gpio::{Input, Level, Output, OutputDrive, Pull};
use embassy_nrf::interrupt::InterruptExt;
use embassy_nrf::{bind_interrupts, pac, peripherals, rng};
use embassy_sync::blocking_mutex::{Mutex, raw::CriticalSectionRawMutex};
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;
use embassy_time::{Instant, Timer};
use kb_component_runtime::{BrailleComponent, BrailleTyping};
use kb_core::braille_hid::{Report, stroke_reports};
use kb_core::hid::BRAILLE_BLE_REPORT_DESCRIPTOR as REPORT_DESCRIPTOR;
use kb_core::preferences::{Output as StrokeBatch, Settings};
use kb_core::transport::FaultLatch;
use kb_core::transport::SessionGate;
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

#[cfg(feature = "display")]
bind_interrupts!(struct DisplayIrqs {
    TWISPI0 => embassy_nrf::twim::InterruptHandler<peripherals::TWISPI0>;
});

static SESSION: Mutex<CriticalSectionRawMutex, Cell<SessionGate>> =
    Mutex::new(Cell::new(SessionGate::new()));
static SESSION_CHANGED: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static STROKES: Channel<CriticalSectionRawMutex, (u32, KeyStroke), 32> = Channel::new();
const DOTS: [u16; 8] = [64, 4, 2, 1, 8, 16, 32, 256];

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
    report_map: [u8; REPORT_DESCRIPTOR.len()],
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
    #[descriptor(uuid = "2908", read, value = [1u8, 1u8])]
    #[characteristic(uuid = "2a4d", read, notify, permissions(encrypted))]
    input_report: [u8; 8],
    #[descriptor(uuid = "2908", read, value = [1u8, 2u8])]
    #[characteristic(
        uuid = "2a4d",
        read,
        write,
        write_without_response,
        permissions(encrypted)
    )]
    output_report: [u8; 1],
    #[descriptor(uuid = "2908", read, value = [2u8, 1u8])]
    #[characteristic(uuid = "2a4d", read, notify, permissions(encrypted))]
    consumer_report: [u8; 2],
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
    embassy_nrf::interrupt::RNG.set_priority(embassy_nrf::interrupt::Priority::P3);
    #[cfg(feature = "display")]
    embassy_nrf::interrupt::TWISPI0.set_priority(embassy_nrf::interrupt::Priority::P3);
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
    let mut component = FaultLatch::new(BrailleComponent::new(include_bytes!(env!(
        "KB_BRAILLE_WASM"
    ))));
    #[cfg(feature = "display")]
    let _display_power = Output::new(p.P0_13, Level::Low, OutputDrive::Standard);
    #[cfg(feature = "display")]
    let mut display_dma = [0; 32];
    #[cfg(feature = "display")]
    let display_bus = embassy_nrf::twim::Twim::new(
        p.TWISPI0,
        DisplayIrqs,
        p.P0_17,
        p.P0_20,
        embassy_nrf::twim::Config::default(),
        &mut display_dma,
    );
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
            bonds.forget().await;
        }
    }
    let saved_bonds = bonds.load_all().unwrap();
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
    for bond in saved_bonds.into_iter().flatten() {
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
        let mut typing = BrailleTyping::new(false);
        let settings = Settings::default();
        let mut pending = StrokeBatch::default();
        let mut fault_seen = false;
        let mut armed = false;
        let mut generation = 0;
        #[cfg(feature = "display")]
        let mut display_status = kb_display::Status {
            transport: kb_display::Transport::Bluetooth,
            ..Default::default()
        };
        loop {
            let raw = inputs
                .iter()
                .enumerate()
                .fold(0, |mask, (i, pin)| mask | (u64::from(pin.is_low()) << i));
            let now = Instant::now().as_millis() as u32;
            let (next, update) = debounce.sample(raw, now);
            debounce = next;
            let session = SESSION.lock(Cell::get);
            let connected = session.ready();
            if !connected || generation != session.generation() {
                STROKES.clear();
                generation = session.generation();
                typing.cancel();
                pending = StrokeBatch::default();
                armed = false;
            } else if update.stable == 0 {
                armed = true;
            }
            if armed && !component.is_faulted() && pending.is_empty() {
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
                pending = typing.step(&mut component, &settings, dots, now);
            }
            if !fault_seen && component.is_faulted() {
                fault_seen = true;
                armed = false;
                pending = StrokeBatch::default();
                STROKES.clear();
                generation = update_session(SessionGate::invalidate).generation();
            }
            let mut sent = 0;
            for &stroke in pending.as_slice() {
                if STROKES.try_send((generation, stroke)).is_err() {
                    break;
                }
                sent += 1;
                #[cfg(feature = "display")]
                {
                    display_status.last_usage = stroke.usage;
                    display_status.last_modifiers = stroke.modifiers;
                }
            }
            if sent != 0 {
                pending =
                    StrokeBatch::from_slice(&pending.as_slice()[sent..], KeyStroke::default())
                        .unwrap();
            }
            #[cfg(feature = "display")]
            {
                let faulted = component.is_faulted();
                let pairing_unsaved = kb_nrf52840::ble_bonds::storage_failed();
                let changed = display_status.pairing_unsaved != pairing_unsaved
                    || display_status.faulted != faulted
                    || display_status.connected != connected
                    || display_status.held != typing.held()
                    || display_status.chord != typing.chord()
                    || sent != 0
                    || (update.pressed | update.released) != 0;
                display_status.pairing_unsaved = pairing_unsaved;
                display_status.faulted = faulted;
                display_status.connected = connected;
                display_status.held = typing.held();
                display_status.chord = typing.chord();
                if changed {
                    kb_nrf52840::display::update(display_status);
                }
            }
            let illuminated = if component.is_faulted() {
                Instant::now().as_millis() / 200 % 2 == 0
            } else if kb_nrf52840::ble_bonds::storage_failed() {
                let phase = Instant::now().as_millis() % 2000;
                phase < 100 || (200..300).contains(&phase)
            } else {
                update.stable != 0
            };
            indicator.set_level(if illuminated { Level::High } else { Level::Low });
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
            update_session(SessionGate::reset);
            STROKES.clear();
            let advertiser = peripheral
                .advertise(
                    &Default::default(),
                    Advertisement::ConnectableScannableUndirected {
                        adv_data: &advertising[..len],
                        scan_data: &[],
                    },
                )
                .await;
            let Ok(advertiser) = advertiser else {
                Timer::after_millis(100).await;
                continue;
            };
            let Ok(connection) = advertiser.accept().await else {
                continue;
            };
            if connection.set_bondable(true).is_err() {
                continue;
            }
            let connection = connection.with_attribute_server(&server).unwrap();
            connection.set(&server.hid.protocol_mode, &1).unwrap();
            connection.set(&server.hid.control_point, &1).unwrap();
            connection.set(&server.hid.input_report, &[0; 8]).unwrap();
            connection.set(&server.hid.boot_input, &[0; 8]).unwrap();
            connection
                .set(&server.hid.consumer_report, &[0; 2])
                .unwrap();
            let _ = select(
                gatt_events(&server, &connection, &mut bonds),
                send_reports(&server, &connection),
            )
            .await;
        }
    };
    let ble = async {
        let _ = join(runner.run(), connections).await;
    };
    let keyboard = join(ble, scan);
    #[cfg(feature = "display")]
    join(keyboard, kb_nrf52840::display::run(display_bus)).await;
    #[cfg(not(feature = "display"))]
    keyboard.await;
}

async fn gatt_events(
    server: &Server<'_>,
    connection: &GattConnection<'_, '_, DefaultPacketPool>,
    bonds: &mut kb_nrf52840::ble_bonds::BondStore<'_>,
) {
    loop {
        match connection.next().await {
            GattConnectionEvent::Disconnected { .. } => return,
            GattConnectionEvent::PairingComplete {
                bond: Some(bond), ..
            } => {
                bonds.remember(&bond).await;
                bonds
                    .remember_client_table(
                        &server.server,
                        connection.raw(),
                        kb_nrf52840::ble_bonds::BRAILLE_HID_SCHEMA,
                    )
                    .await;
            }
            GattConnectionEvent::Encrypted { security_level, .. } if security_level.encrypted() => {
                bonds.restore_client_table(
                    &server.server,
                    connection.raw(),
                    kb_nrf52840::ble_bonds::BRAILLE_HID_SCHEMA,
                );
            }
            GattConnectionEvent::Gatt { event } => {
                let changed = matches!(&event, GattEvent::Write(_));
                let reset = if let GattEvent::Write(ref write) = event {
                    let handle = write.handle();
                    handle == server.hid.protocol_mode.handle
                        || handle == server.hid.control_point.handle
                        || [
                            server.hid.input_report.cccd_handle(),
                            server.hid.boot_input.cccd_handle(),
                            server.hid.consumer_report.cccd_handle(),
                        ]
                        .iter()
                        .flatten()
                        .any(|cccd| cccd.handle() == handle)
                } else {
                    false
                };
                if let Ok(reply) = event.accept() {
                    if reset {
                        update_session(SessionGate::reset);
                        STROKES.clear();
                    }
                    reply.send().await;
                }
                if changed {
                    bonds
                        .remember_client_table(
                            &server.server,
                            connection.raw(),
                            kb_nrf52840::ble_bonds::BRAILLE_HID_SCHEMA,
                        )
                        .await;
                }
            }
            _ => {}
        }
    }
}

fn update_session(update: impl FnOnce(SessionGate) -> SessionGate) -> SessionGate {
    let (previous, next) = SESSION.lock(|state| {
        let previous = state.get();
        let next = update(previous);
        state.set(next);
        (previous, next)
    });
    if previous != next {
        SESSION_CHANGED.signal(());
    }
    next
}

async fn session_changed(generation: u32) {
    while SESSION.lock(Cell::get).generation() == generation {
        SESSION_CHANGED.wait().await;
    }
}

async fn send_reports(server: &Server<'_>, connection: &GattConnection<'_, '_, DefaultPacketPool>) {
    let mut generation = None;
    let mut previous_consumer = [0; 2];
    loop {
        let boot = connection.get(&server.hid.protocol_mode).unwrap_or(1) == 0;
        let keyboard = if boot {
            server.hid.boot_input
        } else {
            server.hid.input_report
        };
        let consumer = server.hid.consumer_report;
        let encrypted = connection
            .raw()
            .security_level()
            .map(|level| level.encrypted())
            .unwrap_or(false);
        let ready = encrypted
            && connection.get(&server.hid.control_point).unwrap_or(1) != 0
            && keyboard.should_notify(connection);
        let session = update_session(|state| state.set_ready(ready));
        let token = session.generation();
        if generation != Some(token) {
            generation = Some(token);
            previous_consumer = [0; 2];
            let release = async {
                for characteristic in [server.hid.boot_input, server.hid.input_report] {
                    connection.set(&characteristic, &[0; 8])?;
                    if encrypted && characteristic.should_notify(connection) {
                        characteristic.notify(connection, &[0; 8], true).await?;
                    }
                }
                connection.set(&consumer, &[0; 2])?;
                if encrypted && consumer.should_notify(connection) {
                    consumer.notify(connection, &[0; 2], true).await?;
                }
                Ok::<(), trouble_host::Error>(())
            };
            match select(session_changed(token), release).await {
                Either::First(()) => continue,
                Either::Second(Err(_)) => return,
                Either::Second(Ok(())) => {}
            }
        }
        if !ready {
            STROKES.clear();
            let _ = select(session_changed(token), Timer::after_millis(10)).await;
            continue;
        }
        if let Ok((queued, stroke)) = STROKES.try_receive() {
            if queued != token {
                continue;
            }
            let consumer_ready = !boot && consumer.should_notify(connection);
            let send = async {
                for report in stroke_reports(previous_consumer, stroke)
                    .into_iter()
                    .flatten()
                {
                    match report {
                        Report::Keyboard(value)
                            if value == [0; 8] || stroke.consumer == 0 || consumer_ready =>
                        {
                            keyboard.notify(connection, &value, true).await?
                        }
                        Report::Keyboard(_) => {}
                        Report::Consumer(value) if consumer_ready => {
                            consumer.notify(connection, &value, true).await?;
                            previous_consumer = value;
                        }
                        Report::Consumer(_) => {}
                    }
                }
                Ok::<(), trouble_host::Error>(())
            };
            match select(session_changed(token), send).await {
                Either::First(()) => continue,
                Either::Second(Err(_)) => return,
                Either::Second(Ok(())) => {}
            }
        }
        let _ = select(session_changed(token), Timer::after_millis(1)).await;
    }
}
