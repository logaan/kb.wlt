use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_time::Instant;
use kb_client_session::{BOOTLOADER_PROSPECTOR, Links, Model, Session};
use kb_component_runtime::DisplayGuest;
use kb_core::{
    KeyStroke,
    host_target::{Target, TargetConnection},
    menu::BluetoothAction,
    pipeline::Bounded,
    preferences::{ENCODED_LEN, MAX_OUTPUT, Settings},
    transport::InputGate,
};

pub static OUTPUT: Channel<CriticalSectionRawMutex, (u32, crate::usb_hid::Output), 64> =
    Channel::new();

pub(crate) fn present(
    client: &mut impl DisplayGuest,
    model: Model,
) -> Result<(), kb_component_runtime::Error> {
    let bytes = client.render(280, 240, model.bytes.as_slice(), model.detail.as_str())?;
    let scene = kb_display::Scene::from_bytes(bytes.as_slice())
        .ok_or(kb_component_runtime::Error::InvalidReturn)?;
    crate::prospector::present(scene);
    Ok(())
}

pub async fn run<G: DisplayGuest>(
    component: &mut Option<G>,
    mut next: impl AsyncFnMut(bool) -> Option<(kb_core::split::Update, Links)>,
) {
    let settings = crate::settings::current()
        .and_then(|blob| Settings::decode(blob.as_slice()).ok())
        .unwrap_or_default();
    let mut app = Session::new(settings);
    app.set_viewport(280, 240);
    app.set_bootloader_devices(BOOTLOADER_PROSPECTOR);
    let mut pending = Bounded::<KeyStroke, 128>::default();
    let mut input = InputGate::new();
    let mut displayed = None;
    loop {
        let accept = 128 - pending.len() >= MAX_OUTPUT;
        let Some((update, mut links)) = next(accept).await else {
            return;
        };
        let usb = crate::usb_hid::connection();
        let mut generation = usb.generation();
        links.target = Target::Usb;
        links.connection = if usb.ready() {
            TargetConnection::Connected
        } else {
            TargetConnection::Offline
        };
        let gate = input.observe(
            generation,
            true,
            u64::from(update.keys),
            update.cancel_chord,
        );
        if gate.cancel {
            OUTPUT.clear();
            pending = Default::default();
            if update.cancel_chord {
                generation = crate::usb_hid::invalidate_pending_reports();
                input.acknowledge_generation(generation);
            }
            app.cancel(update.keys);
        }
        let Some(client) = component.as_mut() else {
            continue;
        };
        let change = if let Ok(command) = crate::prospector_touch::COMMANDS.try_receive() {
            Some(app.navigate_action(command, client))
        } else if gate.keys.is_some() && accept {
            Some(app.update(update.keys, Instant::now().as_millis() as u32, client))
        } else {
            None
        };
        if let Some(change) = change {
            let Ok(change) = change else {
                component.take();
                pending = Default::default();
                OUTPUT.clear();
                crate::usb_hid::invalidate_pending_reports();
                continue;
            };
            if change.bootloader == BOOTLOADER_PROSPECTOR {
                OUTPUT.clear();
                crate::usb_hid::invalidate_pending_reports();
                crate::bootloader::enter_uf2().await;
            }
            if change.reset {
                OUTPUT.clear();
                pending = Default::default();
                generation = crate::usb_hid::invalidate_pending_reports();
                input.acknowledge_generation(generation);
            }
            if usb.ready() {
                pending
                    .extend_from_slice(change.strokes.as_slice())
                    .unwrap();
            }
            if let Some(action) = change.bluetooth {
                let command = match action {
                    BluetoothAction::Reconnect => crate::settings::BluetoothControl::Reconnect,
                    BluetoothAction::ClearBonds => crate::settings::BluetoothControl::ClearBonds,
                };
                if !crate::settings::request_bluetooth(command) {
                    app.busy();
                }
            }
            if change.save {
                let mut bytes = [0; ENCODED_LEN];
                if app.settings.encode(&mut bytes).is_err()
                    || !crate::settings::request_save(&bytes)
                {
                    app.busy();
                }
            }
        }
        let sent = pending
            .as_slice()
            .iter()
            .take_while(|stroke| {
                OUTPUT
                    .try_send((generation, crate::usb_hid::Output::Stroke(**stroke)))
                    .is_ok()
            })
            .count();
        if sent != 0 {
            pending =
                Bounded::from_slice(&pending.as_slice()[sent..], KeyStroke::default()).unwrap();
        }
        let model = app.model(links);
        if displayed != Some(model) && present(client, model).is_ok() {
            displayed = Some(model);
        }
    }
}
