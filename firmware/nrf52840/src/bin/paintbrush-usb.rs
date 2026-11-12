#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_futures::join::join3;
use embassy_futures::select::{Either, select};
use embassy_nrf::gpio::{Input, Level, Output, OutputDrive, Pull};
use embassy_nrf::usb::{Driver, vbus_detect::HardwareVbusDetect};
use embassy_nrf::{bind_interrupts, peripherals, usb};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Instant, Timer};
use embassy_usb::class::hid::{
    Config as HidConfig, HidBootProtocol, HidSubclass, HidWriter, State,
};
use embassy_usb::{Builder, Config};
use kb_component_runtime::{BrailleComponent, BrailleState};
use kb_core::{Debouncer, KeyStroke};
use panic_halt as _;

bind_interrupts!(struct Irqs {
    USBD => usb::InterruptHandler<peripherals::USBD>;
    CLOCK_POWER => usb::vbus_detect::InterruptHandler;
});

#[cfg(feature = "display")]
bind_interrupts!(struct DisplayIrqs {
    TWISPI0 => embassy_nrf::twim::InterruptHandler<peripherals::TWISPI0>;
});

static STROKES: Channel<CriticalSectionRawMutex, (u32, KeyStroke), 32> = Channel::new();
const DOTS: [u16; 8] = [64, 4, 2, 1, 8, 16, 32, 256];
const REPORT_DESCRIPTOR: &[u8] = &[
    0x05, 0x01, 0x09, 0x06, 0xa1, 0x01, 0x05, 0x07, 0x19, 0xe0, 0x29, 0xe7, 0x15, 0x00, 0x25, 0x01,
    0x75, 0x01, 0x95, 0x08, 0x81, 0x02, 0x95, 0x01, 0x75, 0x08, 0x81, 0x01, 0x95, 0x05, 0x75, 0x01,
    0x05, 0x08, 0x19, 0x01, 0x29, 0x05, 0x91, 0x02, 0x95, 0x01, 0x75, 0x03, 0x91, 0x01, 0x95, 0x06,
    0x75, 0x08, 0x15, 0x00, 0x25, 0x65, 0x05, 0x07, 0x19, 0x00, 0x29, 0x65, 0x81, 0x00, 0xc0,
];

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    kb_nrf52840::init_heap();
    let p = embassy_nrf::init(kb_nrf52840::clock_config());
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
    let driver = Driver::new(p.USBD, Irqs, HardwareVbusDetect::new(Irqs));
    let mut config = Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("kb.wlt");
    config.product = Some("Paintbrush Braille");
    config.max_power = 100;
    config.max_packet_size_0 = 64;
    config.composite_with_iads = false;
    let mut config_descriptor = [0; 256];
    let mut bos_descriptor = [0; 64];
    let mut msos_descriptor = [0; 64];
    let mut control_buffer = [0; 64];
    let mut hid_state = State::new();
    let mut control_handler = kb_nrf52840::usb_hid::KeyboardControl::default();
    let mut usb_state = kb_nrf52840::usb_hid::ConnectionState::default();
    let mut builder = Builder::new(
        driver,
        config,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut msos_descriptor,
        &mut control_buffer,
    );
    builder.handler(&mut usb_state);
    let mut writer = HidWriter::<_, 8>::new(
        &mut builder,
        &mut hid_state,
        HidConfig {
            report_descriptor: REPORT_DESCRIPTOR,
            request_handler: Some(&mut control_handler),
            poll_ms: 1,
            max_packet_size: 8,
            hid_subclass: HidSubclass::Boot,
            hid_boot_protocol: HidBootProtocol::Keyboard,
        },
    );
    let mut usb = builder.build();
    let scan = async {
        let mut debounce = Debouncer::<8>::new(5);
        let mut state = BrailleState::default();
        let mut armed = false;
        let mut generation = 0;
        #[cfg(feature = "display")]
        let mut display_status = kb_display::Status::default();
        loop {
            let raw = inputs
                .iter()
                .enumerate()
                .fold(0, |mask, (i, pin)| mask | (u64::from(pin.is_low()) << i));
            let (next, update) = debounce.sample(raw, Instant::now().as_millis() as u32);
            debounce = next;
            let session = kb_nrf52840::usb_hid::connection();
            let configured = session.ready();
            if !configured || generation != session.generation() {
                STROKES.clear();
                generation = session.generation();
                state = BrailleState::default();
                armed = false;
            } else if update.stable == 0 {
                armed = true;
            }
            if armed && (update.pressed | update.released) != 0 {
                let mut dots = 0;
                for position in 0..8 {
                    if update.stable & (1 << position) == 0 {
                        continue;
                    }
                    let dot_index = if cfg!(feature = "paintbrush-left") {
                        position ^ 3
                    } else {
                        position
                    };
                    dots |= DOTS[dot_index];
                }
                let output = component.step(state, dots).unwrap();
                state = output.state;
                if output.usage != 0 {
                    let _ = STROKES
                        .try_send((generation, KeyStroke::new(output.usage, output.modifiers)));
                    #[cfg(feature = "display")]
                    {
                        display_status.last_usage = output.usage;
                        display_status.last_modifiers = output.modifiers;
                    }
                }
            }
            #[cfg(feature = "display")]
            {
                let changed = display_status.connected != configured
                    || display_status.held != state.held
                    || display_status.chord != state.chord
                    || (update.pressed | update.released) != 0;
                display_status.connected = configured;
                display_status.held = state.held;
                display_status.chord = state.chord;
                if changed {
                    kb_nrf52840::display::update(display_status);
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
    let send = async {
        loop {
            writer.ready().await;
            if writer.write(&[0; 8]).await.is_ok() {
                kb_nrf52840::usb_hid::report_sent([0; 8]);
            }
            if let Either::First((generation, stroke)) =
                select(STROKES.receive(), kb_nrf52840::usb_hid::idle_timeout()).await
            {
                if !kb_nrf52840::usb_hid::connection().accepts(generation) {
                    continue;
                }
                if writer.write(&stroke.report()).await.is_ok() {
                    kb_nrf52840::usb_hid::report_sent(stroke.report());
                    if writer.write(&[0; 8]).await.is_ok() {
                        kb_nrf52840::usb_hid::report_sent([0; 8]);
                    }
                }
            }
        }
    };
    let keyboard = join3(usb.run(), scan, send);
    #[cfg(feature = "display")]
    embassy_futures::join::join(keyboard, kb_nrf52840::display::run(display_bus)).await;
    #[cfg(not(feature = "display"))]
    keyboard.await;
}
