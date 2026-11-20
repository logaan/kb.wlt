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

#[cfg(feature = "toucan-dongle")]
pub(super) static REPORTS: Channel<CriticalSectionRawMutex, (u32, crate::usb_hid::Output), 64> =
    Channel::new();

#[cfg(feature = "toucan-display")]
static REPORTS: Channel<CriticalSectionRawMutex, (u32, crate::usb_hid::Output), 4> = Channel::new();

#[cfg(feature = "toucan-display")]
async fn route_usb() {
    use super::route;
    use crate::usb_hid;
    use embassy_futures::select::{Either, select};
    use kb_core::host_target::Target;

    let mut previous_usb = None;
    let mut route_generation = None;
    loop {
        let connection = usb_hid::connection();
        if previous_usb != connection.token() {
            route::set_transport_ready(Target::Usb, false);
            previous_usb = connection.token();
        }
        route::set_transport_ready(Target::Usb, connection.ready());
        if route::target() != Target::Usb || !connection.ready() {
            if route_generation.take().is_some() {
                REPORTS.clear();
                usb_hid::invalidate_pending_reports();
                previous_usb = usb_hid::connection().token();
            }
            Timer::after_millis(10).await;
            continue;
        }
        if let Either::First((generation, output)) =
            select(route::USB_REPORTS.receive(), Timer::after_millis(10)).await
        {
            if route::target() != Target::Usb || !route::connection().accepts(generation) {
                continue;
            }
            if route_generation != Some(generation) {
                REPORTS.clear();
                usb_hid::invalidate_pending_reports();
                previous_usb = usb_hid::connection().token();
                route_generation = Some(generation);
            }
            if let Some(token) = previous_usb {
                let sent = embassy_time::with_timeout(
                    embassy_time::Duration::from_millis(100),
                    REPORTS.send((token, output)),
                )
                .await;
                if sent.is_err() && route::connection().accepts(generation) {
                    route::invalidate_pending_reports();
                }
            }
        }
    }
}

pub(super) async fn run_usb(peripheral: Peri<'static, peripherals::USBD>) {
    let driver = Driver::new(peripheral, Irqs, HardwareVbusDetect::new(Irqs));
    let mut config = Config::new(
        0xc0de,
        if cfg!(feature = "toucan-display") {
            0xcafd
        } else {
            0xcafe
        },
    );
    config.manufacturer = Some("kb.wlt");
    #[cfg(feature = "toucan-dongle")]
    {
        config.product = Some("kb.wlt Toucan Receiver");
    }
    #[cfg(feature = "toucan-display")]
    {
        config.product = Some("kb.wlt Toucan USB");
    }
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
    #[cfg(feature = "toucan-dongle")]
    let send = crate::usb_hid::send_strokes(&mut writer, &mut consumer, &REPORTS);
    #[cfg(feature = "toucan-display")]
    let send = join(
        crate::usb_hid::send_strokes_with_release(
            &mut writer,
            &mut consumer,
            &REPORTS,
            super::route::released,
        ),
        route_usb(),
    );
    join(usb.run(), send).await;
}
