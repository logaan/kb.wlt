use super::*;
use crate::radio::Irqs;
use embassy_nrf::{
    Peri, peripherals,
    usb::{Driver, vbus_detect::HardwareVbusDetect},
};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_usb::{
    Builder, Config,
    class::hid::{Config as HidConfig, HidBootProtocol, HidSubclass, HidWriter, State},
};
use kb_core::hid::KEYBOARD_REPORT_DESCRIPTOR as REPORT_DESCRIPTOR;

pub(super) static REPORTS: Channel<CriticalSectionRawMutex, (u32, crate::usb_hid::Output), 64> =
    Channel::new();

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
