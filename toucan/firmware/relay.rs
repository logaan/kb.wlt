use crate::{
    radio::{Bonds, Host},
    usb_hid::Output,
};
#[cfg(feature = "toucan-display")]
use core::future::Future;
#[cfg(feature = "toucan-display")]
use embassy_futures::select::{Either, select};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::{Duration, Timer, with_timeout};
#[cfg(feature = "toucan-display")]
use kb_client_session::Model;
use kb_client_session::Navigation;
use kb_core::matrix_link::{DISPLAY_PACKET_LEN, DisplayFeedback, DisplayTouch};
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
const BOOTLOADER_UUID: Uuid = Uuid::new_long(uuid_bytes(6));
const TOUCH_UUID: Uuid = Uuid::new_long(uuid_bytes(7));
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
    use crate::toucan::{bootloader_link, route};
    use core::{
        cell::Cell,
        sync::atomic::{AtomicBool, Ordering},
    };
    use embassy_sync::{blocking_mutex::Mutex, channel::Channel};
    use kb_core::host_target::Target;
    use kb_core::pipeline::Bounded;
    use kb_core::preferences::Settings;

    static DOCKED: AtomicBool = AtomicBool::new(false);
    const DOCK_SCHEMA: u32 = 0x5443_444b;
    pub(crate) static BOOTLOADER: bootloader_link::Link = bootloader_link::Link::new();
    pub(crate) fn docked() -> bool {
        DOCKED.load(Ordering::Relaxed)
    }

    pub(crate) fn is_peer(store: &crate::ble_bonds::BondStore<'_>, identity: &Identity) -> bool {
        store.client(identity, DOCK_SCHEMA).is_some()
    }

    struct DockConnection;
    impl Drop for DockConnection {
        fn drop(&mut self) {
            DOCKED.store(false, Ordering::Relaxed);
            TOUCH.clear();
        }
    }

    static VIEW: Mutex<CriticalSectionRawMutex, Cell<Option<Model>>> = Mutex::new(Cell::new(None));
    static CHANGED: Signal<CriticalSectionRawMutex, ()> = Signal::new();
    static TOUCH: Channel<CriticalSectionRawMutex, (Model, Navigation), 4> = Channel::new();

    pub(crate) fn take_touch(model: Model) -> Option<Navigation> {
        while let Ok((source, command)) = TOUCH.try_receive() {
            if source == model && docked() {
                return Some(command);
            }
        }
        None
    }

    pub(crate) fn publish(model: Model) {
        VIEW.lock(|v| v.set(Some(model)));
        CHANGED.signal(());
    }

    pub(crate) async fn advertise(stack: &Host<'_>, bonds: &Bonds) {
        let mut payload = [0; 31];
        let len = AdStructure::encode_slice(
            &[
                AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
                AdStructure::CompleteServiceUuids128(&[uuid_bytes(1)]),
            ],
            &mut payload,
        )
        .unwrap();
        let Some(conn) = crate::radio::accept_peripheral(stack, &payload[..len]).await else {
            return;
        };
        if conn.set_bondable(true).is_err() {
            return;
        }
        let peer_is_hid = {
            let store = bonds.lock().await;
            crate::toucan::ble_hid::is_hid_peer(&store, &conn.peer_identity())
        };
        if peer_is_hid {
            conn.disconnect();
            return;
        }
        if crate::radio::wait_encrypted(&conn, stack, bonds).await {
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
                        } => {
                            let mut store = bonds.lock().await;
                            if docked() {
                                let table = ClientAttTable::builder().build();
                                store
                                    .remember_with_client(&bond, DOCK_SCHEMA, table.raw())
                                    .await;
                            } else {
                                store.remember(&bond).await;
                            }
                        }
                        _ => {}
                    }
                }
            };
            let _ = select(events, send(stack, &conn, bonds)).await;
        }
        route::set_transport_ready(Target::Prospector, false);
        conn.disconnect();
    }

    async fn send(stack: &Host<'_>, conn: &Connection<'_, DefaultPacketPool>, bonds: &Bonds) {
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
            let bootloader =
                request(client.characteristic_by_uuid::<u8>(service, &BOOTLOADER_UUID)).await;
            let mut touch =
                request(client.characteristic_by_uuid::<[u8; 3]>(service, &TOUCH_UUID)).await;
            let boot_session = BOOTLOADER.session();
            DOCKED.store(true, Ordering::Relaxed);
            let _session = DockConnection;
            if conn.is_bonded_peer() {
                let table = ClientAttTable::builder().build();
                bonds
                    .lock()
                    .await
                    .remember_client(&conn.peer_identity(), DOCK_SCHEMA, table.raw())
                    .await;
            }
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
            let mut sequence = 0u16;
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
                        route::set_transport_ready(Target::Prospector, false);
                    }
                    remote = Some(generation);
                    route::set_transport_ready(Target::Prospector, state[0] == 1);
                    if let Some(characteristic) = touch.as_ref() {
                        let mut packet = DisplayTouch::IDLE;
                        let reply =
                            request(client.read_characteristic(characteristic, &mut packet)).await;
                        if reply != Some(packet.len()) {
                            touch = None;
                        } else if let (Some(model), Some(command)) = (
                            last_view,
                            DisplayTouch::decode(packet, sequence.wrapping_sub(1))
                                .and_then(Navigation::from_byte),
                        ) {
                            let _ = TOUCH.try_send((model, command));
                        }
                    }
                    last_poll = embassy_time::Instant::now();
                }
                let model = VIEW.lock(Cell::get);
                if model != last_view && route::PROSPECTOR_REPORTS.is_empty() {
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
                        if BOOTLOADER.available().is_none() {
                            if let Some(characteristic) = bootloader.as_ref() {
                                let mut ready = [0];
                                boot_session.set_ready(
                                    request(client.read_characteristic(characteristic, &mut ready))
                                        .await?
                                        == 1
                                        && ready == [1],
                                );
                            }
                        }
                    }
                    last_view = model;
                }
                match select(
                    route::PROSPECTOR_REPORTS.receive(),
                    select(
                        boot_session.next(),
                        select(CHANGED.wait(), Timer::after_millis(100)),
                    ),
                )
                .await
                {
                    Either::First((token, out)) if route::connection().accepts(token) => {
                        let mut packet = [0; 13];
                        packet[..4].copy_from_slice(&remote?.to_le_bytes());
                        let released = match out {
                            Output::Release(generation) => Some(generation),
                            Output::Held(_) | Output::Stroke(_) => None,
                        };
                        match out {
                            Output::Release(_) => {
                                packet[4] = 1;
                                packet[5..9].copy_from_slice(
                                    &kb_core::KeyStroke::default().held().packed().to_le_bytes(),
                                );
                            }
                            Output::Held(bytes) => packet[5..].copy_from_slice(&bytes),
                            Output::Stroke(stroke) => {
                                packet[4] = 1;
                                packet[5..9].copy_from_slice(&stroke.packed().to_le_bytes());
                            }
                        }
                        request(client.write_characteristic(&hid, &packet)).await?;
                        if let Some(generation) = released {
                            route::released(generation);
                        }
                        poll = false;
                    }
                    Either::Second(Either::First(command)) => {
                        let Some(characteristic) = bootloader.as_ref() else {
                            boot_session.finish(command, false);
                            continue;
                        };
                        boot_session
                            .send(command, client.write_characteristic(characteristic, &[1]))
                            .await;
                        return None;
                    }
                    Either::Second(Either::Second(Either::Second(()))) => poll = true,
                    _ => poll = false,
                }
            }
        };
        let _ = select(client.task(), run).await;
    }
}

#[cfg(feature = "toucan-display")]
pub(super) use left::{BOOTLOADER, advertise, docked, is_peer, publish, take_touch};

#[cfg(feature = "toucan-dongle")]
mod dock {
    use super::*;
    use crate::toucan::receiver::REPORTS;
    use core::{
        cell::Cell,
        sync::atomic::{AtomicBool, Ordering},
    };
    use embassy_futures::{
        join::join,
        select::{Either3, select3},
    };
    use embassy_sync::{blocking_mutex::Mutex, channel::Channel};
    use kb_client_session::Links;
    use kb_component_runtime::{DisplayGuest, KeyboardComponent};
    use kb_core::KeyStroke;
    use kb_core::host_target::{Target, TargetConnection};

    static ADS: Channel<CriticalSectionRawMutex, Address, 4> = Channel::new();
    static VIEW: Signal<CriticalSectionRawMutex, Option<DisplayFeedback>> = Signal::new();
    static TOUCH: Channel<CriticalSectionRawMutex, [u8; 3], 4> = Channel::new();
    static TOUCH_FRAME: Mutex<CriticalSectionRawMutex, Cell<Option<u16>>> =
        Mutex::new(Cell::new(None));
    static TOUCH_SUPPORTED: AtomicBool = AtomicBool::new(false);
    static LINKED: AtomicBool = AtomicBool::new(false);

    static LOCAL_TOUCH: Channel<CriticalSectionRawMutex, Navigation, 4> = Channel::new();

    pub(crate) fn navigate(command: Navigation) {
        if !linked_display() {
            let _ = LOCAL_TOUCH.try_send(command);
            return;
        }
        if TOUCH_SUPPORTED.load(Ordering::Relaxed) {
            if let Some(frame) = TOUCH_FRAME.lock(Cell::get) {
                let _ = TOUCH.try_send(DisplayTouch::encode(frame, command as u8));
            }
        }
    }

    pub(crate) fn linked_display() -> bool {
        LINKED.load(Ordering::Relaxed)
    }

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
        #[characteristic(uuid=BOOTLOADER_UUID,read,write,permissions(encrypted))]
        bootloader: u8,
        #[characteristic(uuid=TOUCH_UUID,value=DisplayTouch::IDLE,read,permissions(encrypted))]
        touch: [u8; 3],
    }

    async fn screen() {
        let Ok(mut guest) = KeyboardComponent::new_embedded() else {
            return;
        };
        let mut standalone = crate::prospector_touch::Standalone::new();
        let mut latest: Option<DisplayFeedback> = None;
        let mut previous = None;
        loop {
            let home = standalone.model(Links {
                online: u8::from(crate::usb_hid::connection().ready()) << 2,
                target: Target::Unknown,
                connection: TargetConnection::Offline,
                ..Default::default()
            });
            let (model, detail) =
                latest
                    .as_ref()
                    .map_or((home.bytes.as_slice(), home.detail.as_str()), |packet| {
                        (
                            packet.model.as_slice(),
                            core::str::from_utf8(packet.detail.as_slice()).unwrap_or(""),
                        )
                    });
            let current = (latest, home);
            if previous != Some(current) {
                if let Ok(bytes) = guest.render(280, 240, model, detail) {
                    if let Some(scene) = kb_display::Scene::from_bytes(bytes.as_slice()) {
                        crate::prospector::present(scene);
                        TOUCH_FRAME.lock(|frame| frame.set(latest.map(|model| model.sequence)));
                        previous = Some(current);
                    }
                }
            }
            match select3(VIEW.wait(), LOCAL_TOUCH.receive(), Timer::after_millis(100)).await {
                Either3::First(next) => {
                    if latest.is_some() != next.is_some() {
                        standalone = crate::prospector_touch::Standalone::new();
                        LOCAL_TOUCH.clear();
                    }
                    latest = next;
                }
                Either3::Second(command) if !linked_display() => {
                    standalone.update(command, &mut guest).await;
                }
                _ => {}
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
            let Ok(scan) = scanner.scan(&config).await else {
                return;
            };
            let candidate = with_timeout(Duration::from_secs(2), ADS.receive()).await;
            scan.stop().await;
            let Ok(addr) = candidate else {
                return;
            };
            addr
        };
        let addresses = [address];
        let config = ConnectConfig {
            scan_config: ScanConfig {
                filter_accept_list: &addresses,
                interval: Duration::from_millis(30),
                window: Duration::from_millis(30),
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
        let mut relaying = false;
        loop {
            match conn.next().await {
                GattConnectionEvent::Disconnected { .. }
                | GattConnectionEvent::PairingFailed(_) => break,
                GattConnectionEvent::RequestConnectionParams(request) => {
                    let _ = request.accept(None, stack).await;
                }
                GattConnectionEvent::PairingComplete {
                    bond: Some(bond), ..
                } => bonds.lock().await.remember(&bond).await,
                GattConnectionEvent::Gatt { event } => {
                    if !conn.raw().security_level().is_ok_and(|s| s.encrypted()) {
                        break;
                    }
                    let reply = match event {
                        GattEvent::Read(read)
                            if read.handle() == server.relay.bootloader.handle =>
                        {
                            let ready = u8::from(relaying && conn.raw().is_bonded_peer());
                            let _ = conn.set(&server.relay.bootloader, &ready);
                            read.accept()
                        }
                        GattEvent::Write(write)
                            if write.handle() == server.relay.bootloader.handle =>
                        {
                            let ready = relaying && conn.raw().is_bonded_peer();
                            crate::bootloader::acknowledge(write, ready, || {
                                REPORTS.clear();
                                crate::usb_hid::invalidate_pending_reports();
                            })
                            .await;
                            continue;
                        }
                        GattEvent::Read(read) if read.handle() == server.relay.touch.handle => {
                            TOUCH_SUPPORTED.store(true, Ordering::Relaxed);
                            let command = TOUCH.try_receive().unwrap_or(DisplayTouch::IDLE);
                            let _ = conn.set(&server.relay.touch, &command);
                            read.accept()
                        }
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
                            relaying = true;
                            LINKED.store(true, Ordering::Relaxed);
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
        LINKED.store(false, Ordering::Relaxed);
        TOUCH_SUPPORTED.store(false, Ordering::Relaxed);
        TOUCH_FRAME.lock(|frame| frame.set(None));
        TOUCH.clear();
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
pub(super) use dock::{Discovery, connect, linked_display, navigate};
