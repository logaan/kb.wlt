use super::{KEY_CHANGED, KEYS, PACKET_LEN, SERVICE_BYTES, SERVICE_UUID, SNAPSHOT_UUID};
use crate::radio::{Bonds, Host};
use core::sync::atomic::Ordering;
use embassy_futures::select::{Either3, select, select3};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_time::{Duration, Instant, Timer, with_timeout};
use kb_core::matrix_link::{Receiver, Side, Snapshot, joined_matrix};
use trouble_host::prelude::*;

static EVENTS: Channel<CriticalSectionRawMutex, LinkEvent, 32> = Channel::new();
static ADVERTISEMENTS: Channel<CriticalSectionRawMutex, Address, 4> = Channel::new();

#[derive(Clone, Copy)]
enum LinkEvent {
    Snapshot {
        snapshot: Snapshot,
        received_at: u32,
        opens: bool,
    },
    Closed,
}

pub(super) struct Discovery;

impl EventHandler for Discovery {
    fn on_adv_reports(&self, mut reports: LeAdvReportsIter<'_>) {
        while let Some(Ok(report)) = reports.next() {
            if advertises_right(report.data) {
                let _ = ADVERTISEMENTS.try_send(Address::new(report.addr_kind, report.addr));
            }
        }
    }
}

fn advertises_right(mut data: &[u8]) -> bool {
    let mut service = false;
    let mut right = false;
    while let Some((&len, remaining)) = data.split_first() {
        if len == 0 {
            break;
        }
        let Some((field, rest)) = remaining.split_at_checked(usize::from(len)) else {
            return false;
        };
        if field[0] == 0x07
            && field[1..]
                .chunks_exact(16)
                .any(|uuid| uuid == SERVICE_BYTES)
        {
            service = true;
        }
        if field.len() == 9 && field[..8] == [0xff, 0xff, 0xff, b'k', b'b', b'w', 2, 2] {
            right = true;
        }
        data = rest;
    }
    service && right
}

pub(super) async fn run(stack: &Host<'_>, bonds: &Bonds) {
    loop {
        while !crate::settings::bluetooth_enabled() {
            Timer::after_millis(100).await;
        }
        let epoch = crate::settings::radio_epoch();
        let _ = select(link(stack, bonds), crate::settings::radio_changed(epoch)).await;
        EVENTS.send(LinkEvent::Closed).await;
        Timer::after_millis(100).await;
    }
}

async fn link(stack: &Host<'_>, bonds: &Bonds) {
    let mut central = stack.central();
    ADVERTISEMENTS.clear();
    let candidate = {
        let mut scanner = Scanner::new(&mut central);
        let config = ScanConfig {
            active: false,
            interval: Duration::from_millis(100),
            window: Duration::from_millis(50),
            ..Default::default()
        };
        let Ok(_scan) = scanner.scan(&config).await else {
            return;
        };
        let Ok(candidate) = with_timeout(Duration::from_secs(1), ADVERTISEMENTS.receive()).await
        else {
            return;
        };
        candidate
    };
    Timer::after_millis(20).await;
    let filter = [candidate];
    let config = ConnectConfig {
        scan_config: ScanConfig {
            filter_accept_list: &filter,
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
    let Ok(Ok(connection)) = with_timeout(Duration::from_secs(10), central.connect(&config)).await
    else {
        return;
    };
    if crate::radio::authenticate(&connection, stack, bonds).await {
        receive_snapshots(stack, bonds, &connection).await;
    }
    connection.disconnect();
}

async fn receive_snapshots(
    stack: &Host<'_>,
    bonds: &Bonds,
    connection: &Connection<'_, DefaultPacketPool>,
) {
    let Ok(Ok(client)) = with_timeout(
        Duration::from_secs(5),
        GattClient::<_, DefaultPacketPool, 12>::new(stack, connection),
    )
    .await
    else {
        return;
    };
    let notifications = async {
        let Ok(Ok(services)) = with_timeout(
            Duration::from_secs(5),
            client.services_by_uuid(&SERVICE_UUID),
        )
        .await
        else {
            return;
        };
        let Some(service) = services.first() else {
            return;
        };
        let Ok(Ok(characteristic)) = with_timeout(
            Duration::from_secs(5),
            client.characteristic_by_uuid::<[u8; PACKET_LEN]>(service, &SNAPSHOT_UUID),
        )
        .await
        else {
            return;
        };
        let Ok(Ok(mut listener)) = with_timeout(
            Duration::from_secs(5),
            client.subscribe(&characteristic, false),
        )
        .await
        else {
            return;
        };
        let mut opens = true;
        loop {
            let Ok(notification) = with_timeout(Duration::from_secs(1), listener.next()).await
            else {
                break;
            };
            let Ok(snapshot) = Snapshot::decode(notification.as_ref()) else {
                continue;
            };
            if snapshot.side != Side::Right {
                break;
            }
            opens = EVENTS
                .try_send(LinkEvent::Snapshot {
                    snapshot,
                    received_at: Instant::now().as_millis() as u32,
                    opens,
                })
                .is_err();
        }
    };
    let events = async {
        loop {
            match connection.next().await {
                ConnectionEvent::Disconnected { .. } | ConnectionEvent::PairingFailed(_) => break,
                ConnectionEvent::PairingComplete {
                    bond: Some(bond), ..
                } => bonds.lock().await.remember(&bond).await,
                ConnectionEvent::RequestConnectionParams(request) => {
                    let _ = request.accept(None, stack).await;
                }
                _ => {}
            }
        }
    };
    let _ = select(client.task(), select(notifications, events)).await;
}

pub(super) struct Matrix {
    right: Receiver,
}

pub(super) struct Sample {
    pub now: u32,
    pub keys: u64,
    pub invalidated: bool,
    pub right_connected: bool,
    pub right_battery: u8,
}

impl Matrix {
    pub(super) const fn new() -> Self {
        Self {
            right: Receiver::new(),
        }
    }

    pub(super) async fn poll(&mut self, accept: bool) -> Sample {
        let event = if accept {
            match select3(EVENTS.receive(), KEY_CHANGED.wait(), Timer::after_millis(5)).await {
                Either3::First(event) => Some(event),
                _ => None,
            }
        } else {
            Timer::after_millis(1).await;
            None
        };
        let now = Instant::now().as_millis() as u32;
        let mut invalidated = self.right.expire(now);
        match event {
            Some(LinkEvent::Snapshot {
                snapshot,
                received_at,
                opens,
            }) => {
                invalidated |= self
                    .right
                    .receive_queued(snapshot, received_at, now, opens)
                    .unwrap_or(false);
            }
            Some(LinkEvent::Closed) => {
                invalidated |= self.right.connected();
                self.right.close();
            }
            None => {}
        }
        Sample {
            now,
            keys: joined_matrix(KEYS.load(Ordering::Relaxed), self.right.keys()),
            invalidated,
            right_connected: self.right.connected(),
            right_battery: self.right.battery(),
        }
    }
}
