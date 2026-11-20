use crate::{
    radio::{Bonds, Host},
    usb_hid::Output,
};
#[cfg(feature = "toucan-display")]
use core::future::Future;
use embassy_futures::select::{Either, select};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::{Duration, Timer, with_timeout};
use kb_client_session::Model;
use kb_core::matrix_link::{DISPLAY_PACKET_LEN, DisplayFeedback};
use trouble_host::prelude::*;

const fn uuid_bytes(id: u8) -> [u8; 16] {
    [
        id, 0x09, 0x12, 0x11, 0x4b, 0x57, 0xa7, 0x98, 0x6c, 0xaa, 0x03, 0xaf, 0x22, 0x00, 0x17,
        0x7a,
    ]
}
const SERVICE: Uuid = Uuid::new_long(uuid_bytes(1));
const HID: Uuid = Uuid::new_long(uuid_bytes(2));
const DISPLAY: Uuid = Uuid::new_long(uuid_bytes(3));
const USB: Uuid = Uuid::new_long(uuid_bytes(4));
const PREFERENCES: Uuid = Uuid::new_long(uuid_bytes(5));
use kb_core::preferences::ENCODED_LEN;

#[cfg(feature = "toucan-display")]
async fn request<T, E>(operation: impl Future<Output = Result<T, E>>) -> Option<T> {
    with_timeout(Duration::from_secs(3), operation)
        .await
        .ok()?
        .ok()
}

#[cfg(feature = "toucan-display")]
mod left {
    use super::*;
    use crate::toucan::route;
    use core::{
        cell::Cell,
        sync::atomic::{AtomicBool, Ordering},
    };
    use embassy_sync::blocking_mutex::Mutex;
    use kb_core::pipeline::Bounded;
    use kb_core::preferences::Settings;

    static DOCKED: AtomicBool = AtomicBool::new(false);
    pub(crate) fn docked() -> bool {
        DOCKED.load(Ordering::Relaxed)
    }
    struct DockConnection;
    impl Drop for DockConnection {
        fn drop(&mut self) {
            DOCKED.store(false, Ordering::Relaxed);
        }
    }

    static VIEW: Mutex<CriticalSectionRawMutex, Cell<Option<Model>>> = Mutex::new(Cell::new(None));
    static CHANGED: Signal<CriticalSectionRawMutex, ()> = Signal::new();

    pub(crate) fn publish(model: Model) {
        VIEW.lock(|v| v.set(Some(model)));
        CHANGED.signal(());
    }

    pub(crate) async fn advertise(stack: &Host<'_>, bonds: &Bonds) {
        let mut peripheral = stack.peripheral();
        let mut payload = [0; 31];
        let len = AdStructure::encode_slice(
            &[
                AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
                AdStructure::CompleteServiceUuids128(&[uuid_bytes(1)]),
            ],
            &mut payload,
        )
        .unwrap();
        let Ok(ad) = peripheral
            .advertise(
                &Default::default(),
                Advertisement::ConnectableScannableUndirected {
                    adv_data: &payload[..len],
                    scan_data: &[],
                },
            )
            .await
        else {
            Timer::after_millis(100).await;
            return;
        };
        let Ok(conn) = ad.accept().await else { return };
        if crate::radio::authenticate(&conn, stack, bonds).await {
            let events = async {
                loop {
                    match conn.next().await {
                        ConnectionEvent::Disconnected { .. }
                        | ConnectionEvent::PairingFailed { .. } => break,
                        ConnectionEvent::RequestConnectionParams(req) => {
                            let _ = req.accept(None, stack).await;
                        }
                        ConnectionEvent::PairingComplete {
                            bond: Some(bond), ..
                        } => bonds.lock().await.remember(&bond).await,
                        _ => {}
                    }
                }
            };
            let _ = select(events, send(stack, &conn)).await;
        }
        route::set_ready(false);
        conn.disconnect();
    }

    async fn send(stack: &Host<'_>, conn: &Connection<'_, DefaultPacketPool>) {
        let Ok(Ok(client)) = with_timeout(
            Duration::from_secs(5),
            GattClient::<_, DefaultPacketPool, 8>::new(stack, conn),
        )
        .await
        else {
            return;
        };
        let run = async {
            let services = request(client.services_by_uuid(&SERVICE)).await?;
            let service = services.first()?;
            let hid = request(client.characteristic_by_uuid::<[u8; 13]>(service, &HID)).await?;
            let display = request(
                client.characteristic_by_uuid::<[u8; DISPLAY_PACKET_LEN]>(service, &DISPLAY),
            )
            .await?;
            let usb = request(client.characteristic_by_uuid::<[u8; 5]>(service, &USB)).await?;
            DOCKED.store(true, Ordering::Relaxed);
            let _session = DockConnection;
            if crate::settings::current().is_none() {
                if let Some(source) = request(
                    client.characteristic_by_uuid::<[u8; ENCODED_LEN]>(service, &PREFERENCES),
                )
                .await
                {
                    let mut bytes = [0; ENCODED_LEN];
                    if request(client.read_characteristic(&source, &mut bytes)).await
                        == Some(ENCODED_LEN)
                    {
                        if let Ok(settings) = Settings::decode(&bytes) {
                            route::import(settings);
                        }
                    }
                }
            }
            let mut remote = None;
            let mut last_view = None;
            let mut sequence = 0;
            let mut poll = true;
            let mut last_poll = embassy_time::Instant::now();
            loop {
                if poll || last_poll.elapsed() >= Duration::from_millis(100) {
                    let mut state = [0; 5];
                    if request(client.read_characteristic(&usb, &mut state)).await? != state.len() {
                        return None::<()>;
                    }
                    let generation = u32::from_le_bytes(state[1..5].try_into().unwrap());
                    if remote.is_some_and(|prev| prev != generation) {
                        route::set_ready(false);
                    }
                    remote = Some(generation);
                    route::set_ready(state[0] == 1);
                    last_poll = embassy_time::Instant::now();
                }
                let model = VIEW.lock(Cell::get);
                if model != last_view && route::OUTPUT.is_empty() {
                    if let Some(model) = model {
                        let packet = DisplayFeedback {
                            sequence,
                            model: model.bytes,
                            detail: Bounded::from_slice(model.detail.as_str().as_bytes(), 0)
                                .unwrap(),
                        };
                        request(client.write_characteristic(&display, packet.encode().as_slice()))
                            .await?;
                        sequence = sequence.wrapping_add(1);
                    }
                    last_view = model;
                }
                match select(
                    route::OUTPUT.receive(),
                    select(CHANGED.wait(), Timer::after_millis(100)),
                )
                .await
                {
                    Either::First((token, out)) if route::connection().accepts(token) => {
                        let mut packet = [0; 13];
                        packet[..4].copy_from_slice(&remote?.to_le_bytes());
                        match out {
                            Output::Held(bytes) => packet[5..].copy_from_slice(&bytes),
                            Output::Stroke(stroke) => {
                                packet[4] = 1;
                                packet[5..9].copy_from_slice(&stroke.packed().to_le_bytes());
                            }
                        }
                        request(client.write_characteristic(&hid, &packet)).await?;
                        poll = false;
                    }
                    Either::Second(Either::Second(())) => poll = true,
                    _ => poll = false,
                }
            }
        };
        let _ = select(client.task(), run).await;
    }
}

#[cfg(feature = "toucan-display")]
pub(super) use left::{advertise, docked, publish};

#[cfg(feature = "toucan-dongle")]
mod dock {
    use super::*;
    use crate::toucan::receiver::REPORTS;
    use embassy_futures::join::join;
    use embassy_sync::channel::Channel;
    use kb_client_session::Links;
    use kb_component_runtime::{DisplayGuest, KeyboardComponent};
    use kb_core::KeyStroke;

    static ADS: Channel<CriticalSectionRawMutex, Address, 4> = Channel::new();
    static VIEW: Signal<CriticalSectionRawMutex, Option<DisplayFeedback>> = Signal::new();

    pub(crate) struct Discovery;
    impl EventHandler for Discovery {
        fn on_adv_reports(&self, mut reports: LeAdvReportsIter<'_>) {
            while let Some(Ok(report)) = reports.next() {
                let mut fields = report.data;
                while let Some((&len, rest)) = fields.split_first() {
                    if len == 0 {
                        break;
                    }
                    let Some((field, tail)) = rest.split_at_checked(len.into()) else {
                        break;
                    };
                    if field[0] == 7
                        && field[1..]
                            .chunks_exact(16)
                            .any(|uuid| uuid == uuid_bytes(1))
                    {
                        let _ = ADS.try_send(Address::new(report.addr_kind, report.addr));
                        break;
                    }
                    fields = tail;
                }
            }
        }
    }

    #[gatt_server(connections_max = 1, attribute_table_size = 32)]
    struct RelayServer {
        relay: RelayService,
    }

    #[gatt_service(uuid=SERVICE)]
    struct RelayService {
        #[characteristic(uuid=HID,write,permissions(encrypted))]
        hid: [u8; 13],
        #[characteristic(uuid=DISPLAY,value=[0u8;DISPLAY_PACKET_LEN],write,permissions(encrypted))]
        display: [u8; DISPLAY_PACKET_LEN],
        #[characteristic(uuid=USB,read,permissions(encrypted))]
        usb: [u8; 5],
        #[characteristic(uuid=PREFERENCES,value=[0u8;ENCODED_LEN],read,permissions(encrypted))]
        preferences: [u8; ENCODED_LEN],
    }

    async fn screen() {
        let Ok(mut guest) = KeyboardComponent::new_embedded() else {
            return;
        };
        let mut latest: Option<DisplayFeedback> = None;
        let mut previous = None;
        loop {
            let home = Model::home(Links {
                online: u8::from(crate::usb_hid::connection().ready()) << 2,
                ..Default::default()
            });
            let (model, detail) = latest
                .as_ref()
                .map_or((home.bytes.as_slice(), ""), |packet| {
                    (
                        packet.model.as_slice(),
                        core::str::from_utf8(packet.detail.as_slice()).unwrap_or(""),
                    )
                });
            let current = (latest, home.bytes);
            if previous != Some(current) {
                if let Ok(bytes) = guest.render(280, 240, model, detail) {
                    if let Some(scene) = kb_display::Scene::from_bytes(bytes.as_slice()) {
                        crate::prospector::present(scene);
                        previous = Some(current);
                    }
                }
            }
            if let Either::First(next) = select(VIEW.wait(), Timer::after_millis(100)).await {
                latest = next;
            }
        }
    }

    pub(crate) async fn connect(stack: &Host<'_>, bonds: &Bonds) {
        let radio = async {
            let server = RelayServer::new_with_config(GapConfig::Peripheral(PeripheralConfig {
                name: "kb.wlt Prospector",
                appearance: &appearance::human_interface_device::KEYBOARD,
            }))
            .unwrap();
            if let Some(blob) = crate::settings::current() {
                if let Ok(bytes) = <&[u8; ENCODED_LEN]>::try_from(blob.as_slice()) {
                    let _ = server.set(&server.relay.preferences, bytes);
                }
            }
            loop {
                listen(stack, bonds, &server).await;
                Timer::after_millis(100).await;
            }
        };
        join(radio, screen()).await;
    }

    async fn listen(stack: &Host<'_>, bonds: &Bonds, server: &RelayServer<'_>) {
        let mut central = stack.central();
        ADS.clear();
        let address = {
            let mut scanner = Scanner::new(&mut central);
            let config = ScanConfig {
                active: false,
                interval: Duration::from_millis(70),
                window: Duration::from_millis(40),
                ..Default::default()
            };
            let Ok(_scan) = scanner.scan(&config).await else {
                return;
            };
            let Ok(addr) = with_timeout(Duration::from_secs(2), ADS.receive()).await else {
                return;
            };
            addr
        };
        Timer::after_millis(20).await;
        let addresses = [address];
        let config = ConnectConfig {
            scan_config: ScanConfig {
                filter_accept_list: &addresses,
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
        let Ok(Ok(conn)) = with_timeout(Duration::from_secs(6), central.connect(&config)).await
        else {
            return;
        };
        if !crate::radio::authenticate(&conn, stack, bonds).await {
            conn.disconnect();
            return;
        }
        let Ok(conn) = conn.with_attribute_server(&server.server) else {
            return;
        };
        loop {
            match conn.next().await {
                GattConnectionEvent::Disconnected { .. } => break,
                GattConnectionEvent::PairingComplete {
                    bond: Some(bond), ..
                } => bonds.lock().await.remember(&bond).await,
                GattConnectionEvent::Gatt { event } => {
                    if !conn.raw().security_level().is_ok_and(|s| s.encrypted()) {
                        break;
                    }
                    let reply = match event {
                        GattEvent::Read(read) if read.handle() == server.relay.usb.handle => {
                            let usb = crate::usb_hid::connection();
                            let mut status = [0; 5];
                            status[0] = u8::from(usb.ready());
                            status[1..].copy_from_slice(&usb.generation().to_le_bytes());
                            let _ = server.set(&server.relay.usb, &status);
                            read.accept()
                        }
                        GattEvent::Write(write)
                            if write.handle() == server.relay.display.handle =>
                        {
                            let Some(model) = write.with_data(|offset, bytes| {
                                (offset == 0)
                                    .then(|| DisplayFeedback::decode(bytes).ok())
                                    .flatten()
                            }) else {
                                break;
                            };
                            VIEW.signal(Some(model));
                            write.accept_unprocessed()
                        }
                        GattEvent::Write(write) if write.handle() == server.relay.hid.handle => {
                            let Some((generation, out)) = write.with_data(|offset, bytes| {
                                (offset == 0).then(|| decode_output(bytes)).flatten()
                            }) else {
                                break;
                            };
                            if crate::usb_hid::connection().accepts(generation)
                                && with_timeout(
                                    Duration::from_millis(250),
                                    REPORTS.send((generation, out)),
                                )
                                .await
                                .is_err()
                            {
                                break;
                            }
                            write.accept_unprocessed()
                        }
                        event => event.accept(),
                    };
                    if let Ok(reply) = reply {
                        reply.send().await;
                    }
                }
                _ => {}
            }
        }
        conn.raw().disconnect();
        REPORTS.clear();
        crate::usb_hid::invalidate_pending_reports();
        VIEW.signal(None);
    }

    fn decode_output(bytes: &[u8]) -> Option<(u32, Output)> {
        let bytes: &[u8; 13] = bytes.try_into().ok()?;
        let generation = u32::from_le_bytes(bytes[..4].try_into().ok()?);
        let out = match bytes[4] {
            0 => Output::Held(bytes[5..].try_into().ok()?),
            1 => {
                let packed = u32::from_le_bytes(bytes[5..9].try_into().ok()?);
                let stroke = if packed >> 30 == 1 {
                    KeyStroke::new(packed as u8, (packed >> 8) as u8)
                        .with_consumer((packed >> 16) as u16 & 0x3fff)
                        .held()
                } else {
                    KeyStroke::from_packed(packed)?
                };
                Output::Stroke(stroke)
            }
            _ => return None,
        };
        Some((generation, out))
    }
}

#[cfg(feature = "toucan-dongle")]
pub(super) use dock::{Discovery, connect};
