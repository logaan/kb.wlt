#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_futures::join::join4;
use embassy_nrf::config::{HfclkSource, LfclkSource};
use embassy_nrf::gpio::{Level, Output, OutputDrive};
use embassy_nrf::usb::{Driver, vbus_detect::HardwareVbusDetect};
use embassy_nrf::{bind_interrupts, peripherals, usb};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Instant, Timer};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::{Builder, Config, Handler};
use kb_resetter::{Command, Controller, LineParser, Response};
use panic_halt as _;

bind_interrupts!(struct Irqs {
    USBD => usb::InterruptHandler<peripherals::USBD>;
    CLOCK_POWER => usb::vbus_detect::InterruptHandler;
});

static COMMANDS: Channel<CriticalSectionRawMutex, Command, 8> = Channel::new();
static RESPONSES: Channel<CriticalSectionRawMutex, Response, 32> = Channel::new();

struct UsbState;

impl Handler for UsbState {
    fn configured(&mut self, configured: bool) {
        if !configured {
            COMMANDS.clear();
            RESPONSES.clear();
        }
    }

    fn reset(&mut self) {
        COMMANDS.clear();
        RESPONSES.clear();
    }

    fn enabled(&mut self, enabled: bool) {
        if !enabled {
            COMMANDS.clear();
            RESPONSES.clear();
        }
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let mut config = embassy_nrf::config::Config::default();
    config.hfclk_source = HfclkSource::ExternalXtal;
    config.lfclk_source = LfclkSource::ExternalXtal;
    let p = embassy_nrf::init(config);
    let mut outputs = [
        Output::new(p.P0_02, Level::High, OutputDrive::Standard0Disconnect1),
        Output::new(p.P0_03, Level::High, OutputDrive::Standard0Disconnect1),
    ];
    let driver = Driver::new(p.USBD, Irqs, HardwareVbusDetect::new(Irqs));
    let mut config = Config::new(0xc0de, 0xcafd);
    config.manufacturer = Some("kb.wlt");
    config.product = Some("kb.wlt Reset Controller");
    config.serial_number = Some("KB-RESETTER-01");
    config.max_power = 100;
    config.max_packet_size_0 = 64;
    config.device_class = 0xef;
    config.device_sub_class = 0x02;
    config.device_protocol = 0x01;
    config.composite_with_iads = true;
    let mut config_descriptor = [0; 256];
    let mut bos_descriptor = [0; 64];
    let mut msos_descriptor = [0; 64];
    let mut control_buffer = [0; 64];
    let mut cdc_state = State::new();
    let mut usb_state = UsbState;
    let mut builder = Builder::new(
        driver,
        config,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut msos_descriptor,
        &mut control_buffer,
    );
    builder.handler(&mut usb_state);
    let cdc = CdcAcmClass::new(&mut builder, &mut cdc_state, 64);
    let (mut sender, mut receiver) = cdc.split();
    let mut usb = builder.build();
    let receive = async {
        loop {
            receiver.wait_connection().await;
            let mut parser = LineParser::new();
            let mut packet = [0; 64];
            while let Ok(length) = receiver.read_packet(&mut packet).await {
                for byte in &packet[..length] {
                    match parser.push(*byte) {
                        Some(Ok(command)) => {
                            if COMMANDS.try_send(command).is_err() {
                                let _ = RESPONSES.try_send(Response::QueueFull);
                            }
                        }
                        Some(Err(error)) => {
                            let _ = RESPONSES.try_send(Response::Error(error));
                        }
                        None => {}
                    }
                }
            }
        }
    };
    let control = async {
        let mut controller = Controller::new();
        loop {
            let now = Instant::now().as_millis() as u32;
            let (next, completed) = controller.advance(now);
            controller = next;
            for command in completed.into_iter().flatten() {
                let _ = RESPONSES.try_send(Response::Complete(command));
            }
            if let Ok(command) = COMMANDS.try_receive() {
                let response = match command {
                    Command::Ping => Response::Pong,
                    Command::Status => Response::Status(controller.phases()),
                    Command::Pulse { target, operation } => {
                        match controller.start(target, operation, now) {
                            Ok(next) => {
                                controller = next;
                                Response::Accepted(command)
                            }
                            Err(_) => Response::Busy(command),
                        }
                    }
                };
                let _ = RESPONSES.try_send(response);
            }
            for (output, phase) in outputs.iter_mut().zip(controller.phases()) {
                if phase.pulls_low() {
                    output.set_low();
                } else {
                    output.set_high();
                }
            }
            Timer::after_millis(1).await;
        }
    };
    let send = async {
        loop {
            sender.wait_connection().await;
            let response = RESPONSES.receive().await;
            let mut packet = [0; 64];
            if let Ok(length) = response.encode(&mut packet) {
                let _ = sender.write_packet(&packet[..length]).await;
            }
        }
    };
    join4(usb.run(), receive, control, send).await;
}
