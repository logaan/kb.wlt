use super::route;
use crate::{
    radio::{Bonds, Host},
    usb_hid::Output,
};
use embassy_futures::select::{Either, select};
use embassy_time::{Duration, Timer, with_timeout};
use kb_core::{
    braille_hid::{Report, stroke_reports},
    hid::BRAILLE_BLE_REPORT_DESCRIPTOR,
};
use trouble_host::prelude::*;

const HID_SCHEMA: u32 = 0x5443_4849;
const NAME: &str = "kb.wlt Toucan";

#[gatt_server(connections_max = 1, attribute_table_size = 48)]
struct KeyboardServer {
    hid: HidService,
    battery: BatteryService,
    _device: DeviceInformation,
}

#[gatt_service(uuid = "1812")]
struct HidService {
    #[characteristic(uuid = "2a4a", read, value = [0x11, 0x01, 0x00, 0x02])]
    info: [u8; 4],
    #[characteristic(uuid = "2a4b", read, permissions(encrypted), value = BRAILLE_BLE_REPORT_DESCRIPTOR)]
    report_map: [u8; 92],
    #[characteristic(
        uuid = "2a4e",
        read,
        write_without_response,
        permissions(encrypted),
        value = 1u8
    )]
    protocol: u8,
    #[characteristic(
        uuid = "2a4c",
        write_without_response,
        permissions(encrypted),
        value = 1u8
    )]
    control_point: u8,
    #[descriptor(uuid = "2908", read, value = [1, 1])]
    #[characteristic(uuid = "2a4d", read, notify, permissions(encrypted), value = [0; 8])]
    keyboard: [u8; 8],
    #[descriptor(uuid = "2908", read, value = [1, 2])]
    #[characteristic(
        uuid = "2a4d",
        read,
        write,
        write_without_response,
        permissions(encrypted),
        value = 0u8
    )]
    leds: u8,
    #[descriptor(uuid = "2908", read, value = [2, 1])]
    #[characteristic(uuid = "2a4d", read, notify, permissions(encrypted), value = [0; 2])]
    consumer: [u8; 2],
    #[characteristic(uuid = "2a22", read, notify, permissions(encrypted), value = [0; 8])]
    boot_keyboard: [u8; 8],
    #[characteristic(
        uuid = "2a32",
        read,
        write,
        write_without_response,
        permissions(encrypted),
        value = 0u8
    )]
    boot_leds: u8,
}

#[gatt_service(uuid = "180f")]
struct BatteryService {
    #[characteristic(uuid = "2a19", read, notify)]
    level: u8,
}

#[gatt_service(uuid = "180a")]
struct DeviceInformation {
    #[characteristic(uuid = "2a50", read, value = [0x02, 0xde, 0xc0, 0xfe, 0xca, 0x20, 0x00])]
    pnp_id: [u8; 7],
}

struct Ready;

impl Drop for Ready {
    fn drop(&mut self) {
        route::set_ready(false);
    }
}

pub(super) async fn advertise(stack: &Host<'_>, bonds: &Bonds) {
    let server = KeyboardServer::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: NAME,
        appearance: &appearance::human_interface_device::KEYBOARD,
    }))
    .unwrap();
    let mut peripheral = stack.peripheral();
    let mut data = [0; 31];
    let len = AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::CompleteServiceUuids16(&[[0x12, 0x18]]),
            AdStructure::Unknown {
                ty: 0x19,
                data: &appearance::human_interface_device::KEYBOARD.to_le_bytes(),
            },
            AdStructure::CompleteLocalName(NAME.as_bytes()),
        ],
        &mut data,
    )
    .unwrap();
    loop {
        while !crate::settings::bluetooth_enabled() {
            Timer::after_millis(100).await;
        }
        let epoch = crate::settings::radio_epoch();
        let _ = select(
            async {
                let Ok(advertisement) = peripheral
                    .advertise(
                        &AdvertisementParameters::default(),
                        Advertisement::ConnectableScannableUndirected {
                            adv_data: &data[..len],
                            scan_data: &[],
                        },
                    )
                    .await
                else {
                    return;
                };
                let Ok(connection) = advertisement.accept().await else {
                    return;
                };
                if connection.set_bondable(true).is_err() {
                    return;
                }
                let Ok(connection) = connection.with_attribute_server(&server) else {
                    return;
                };
                let _ = connection.set(&server.hid.protocol, &1);
                let _ = connection.set(&server.hid.control_point, &1);
                let _ = connection.set(&server.hid.keyboard, &[0; 8]);
                let _ = connection.set(&server.hid.boot_keyboard, &[0; 8]);
                let _ = connection.set(&server.hid.consumer, &[0; 2]);
                if connection.raw().request_security().is_err() {
                    return;
                }
                let _ready = Ready;
                let _ = select(
                    events(&server, &connection, stack, bonds),
                    reports(&server, &connection),
                )
                .await;
                connection.raw().disconnect();
            },
            crate::settings::radio_changed(epoch),
        )
        .await;
        Timer::after_millis(100).await;
    }
}

async fn events(
    server: &KeyboardServer<'_>,
    connection: &GattConnection<'_, '_, DefaultPacketPool>,
    stack: &Host<'_>,
    bonds: &Bonds,
) {
    loop {
        match connection.next().await {
            GattConnectionEvent::Disconnected { .. } | GattConnectionEvent::PairingFailed(_) => {
                return;
            }
            GattConnectionEvent::PairingComplete {
                bond: Some(bond), ..
            } => {
                let mut store = bonds.lock().await;
                store.remember(&bond).await;
                store
                    .remember_client_table(&server.server, connection.raw(), HID_SCHEMA)
                    .await;
            }
            GattConnectionEvent::Encrypted { security_level, .. } if security_level.encrypted() => {
                bonds.lock().await.restore_client_table(
                    &server.server,
                    connection.raw(),
                    HID_SCHEMA,
                );
            }
            GattConnectionEvent::RequestConnectionParams(request) => {
                let _ = request.accept(None, stack).await;
            }
            GattConnectionEvent::Gatt { event } => {
                let event = match event {
                    GattEvent::Write(write)
                        if [server.hid.protocol.handle, server.hid.control_point.handle]
                            .contains(&write.handle()) =>
                    {
                        if !write.with_data(|offset, bytes| offset == 0 && matches!(bytes, [0 | 1]))
                        {
                            if let Ok(reply) = write.reject(AttErrorCode::VALUE_NOT_ALLOWED) {
                                reply.send().await;
                            }
                            continue;
                        }
                        GattEvent::Write(write)
                    }
                    event => event,
                };
                let preferences_changed = matches!(&event, GattEvent::Write(write) if
                    [server.hid.keyboard.cccd_handle, server.hid.boot_keyboard.cccd_handle,
                     server.hid.consumer.cccd_handle, server.battery.level.cccd_handle]
                        .contains(&Some(write.handle())));
                if let Ok(reply) = event.accept() {
                    reply.send().await;
                }
                if preferences_changed {
                    bonds
                        .lock()
                        .await
                        .remember_client_table(&server.server, connection.raw(), HID_SCHEMA)
                        .await;
                }
            }
            _ => {}
        }
    }
}

async fn reports(
    server: &KeyboardServer<'_>,
    connection: &GattConnection<'_, '_, DefaultPacketPool>,
) {
    let mut battery = 0xff;
    let mut generation = None;
    let mut protocol = 1;
    let mut consumer = [0; 2];
    loop {
        let measured = crate::battery::level();
        if measured <= 100 && measured != battery {
            if server
                .battery
                .level
                .notify(connection, &measured, true)
                .await
                .is_err()
            {
                return;
            }
            battery = measured;
        }
        let mode = connection.get(&server.hid.protocol).unwrap_or(1);
        if protocol != mode {
            route::set_ready(false);
            protocol = mode;
        }
        let boot = protocol == 0;
        let keyboard = if boot {
            &server.hid.boot_keyboard
        } else {
            &server.hid.keyboard
        };
        let ready = connection
            .raw()
            .security_level()
            .is_ok_and(|level| level.encrypted())
            && connection.get(&server.hid.control_point).unwrap_or(1) != 0
            && keyboard.should_notify(connection);
        route::set_ready(ready);
        let Some(token) = route::connection().token() else {
            generation = None;
            Timer::after_millis(20).await;
            continue;
        };
        if generation != Some(token) {
            if !send(server, connection, token, boot, Report::Keyboard([0; 8])).await
                || !send(server, connection, token, boot, Report::Consumer([0; 2])).await
            {
                Timer::after_millis(20).await;
                continue;
            }
            generation = Some(token);
            consumer = [0; 2];
        }
        let Either::First((output_token, output)) =
            select(route::OUTPUT.receive(), Timer::after_millis(20)).await
        else {
            continue;
        };
        if output_token != token {
            continue;
        }
        let next = match output {
            Output::Held(bytes) => [Some(Report::Keyboard(bytes)), None, None, None],
            Output::Stroke(stroke) => stroke_reports(consumer, stroke),
        };
        for report in next.into_iter().flatten() {
            if !send(server, connection, token, boot, report).await {
                route::invalidate_pending_reports();
                break;
            }
            if let Report::Consumer(bytes) = report {
                consumer = bytes;
            }
        }
    }
}

async fn send(
    server: &KeyboardServer<'_>,
    connection: &GattConnection<'_, '_, DefaultPacketPool>,
    token: u32,
    boot: bool,
    report: Report,
) -> bool {
    if !route::connection().accepts(token) {
        return false;
    }
    let notify = async {
        match report {
            Report::Keyboard(bytes) if boot => {
                server
                    .hid
                    .boot_keyboard
                    .notify(connection, &bytes, true)
                    .await
            }
            Report::Keyboard(bytes) => server.hid.keyboard.notify(connection, &bytes, true).await,
            Report::Consumer(bytes) if !boot => {
                server.hid.consumer.notify(connection, &bytes, true).await
            }
            Report::Consumer(_) => Ok(()),
        }
    };
    matches!(
        with_timeout(Duration::from_millis(100), notify).await,
        Ok(Ok(()))
    )
}
