use super::{relay, right_link, route};
use crate::usb_hid::Output;
use kb_client_session::{
    BOOTLOADER_LEFT, BOOTLOADER_PROSPECTOR, BOOTLOADER_RIGHT, Change, Links, Model, Session,
};
use kb_component_runtime::{DisplayGuest, Error, KeyboardComponent};
use kb_core::{
    matrix_link::{BrailleSwitch, braille_keys, braille_transition_blocked},
    menu::BluetoothAction,
    pipeline::{Bounded, Input, KeyEvent, KeyState},
    report_state::ReportState,
    transport::InputGate,
};

struct Base {
    state: Bounded<u8, 64>,
    reports: ReportState<42>,
    previous: u64,
    wake: Option<u32>,
}

impl Base {
    fn new() -> Self {
        Self {
            state: Default::default(),
            reports: ReportState::new(),
            previous: 0,
            wake: None,
        }
    }

    async fn process(
        &mut self,
        client: &KeyboardComponent,
        keys: u64,
        now: u32,
    ) -> Result<(), Error> {
        if self.previous == keys && !self.wake.is_some_and(|at| now.wrapping_sub(at) as i32 >= 0) {
            return Ok(());
        }
        let mut input = Input::<42>::tick(now);
        for position in 0..42 {
            let bit = 1 << position;
            if (self.previous ^ keys) & bit != 0 {
                input
                    .events
                    .push(KeyEvent {
                        position,
                        state: if keys & bit != 0 {
                            KeyState::Pressed
                        } else {
                            KeyState::Released
                        },
                        timestamp_ms: now,
                    })
                    .unwrap();
            }
        }
        let output = client.process::<64, 42, 42>(self.state.as_slice(), &input)?;
        let transformed = client
            .transform_output::<64, 42>(output.state.as_slice(), output.actions.as_slice())?;
        let mut checked = self.reports;
        for action in transformed.actions.as_slice() {
            checked.apply(*action).map_err(|_| Error::InvalidReturn)?;
        }
        self.previous = keys;
        self.wake = output.wake_at_ms;
        self.state = transformed.state;
        let gate = route::connection();
        for action in transformed.actions.as_slice() {
            let update = self.reports.apply(*action).unwrap();
            for report in [Some(update.first), update.second].into_iter().flatten() {
                if gate.ready() && !route::send(gate.generation(), Output::Held(report)).await {
                    return Ok(());
                }
            }
        }
        Ok(())
    }
}

#[embassy_executor::task]
pub(super) async fn process_keys() {
    let mut client = KeyboardComponent::new_embedded().ok();
    super::COMPONENT_FAILED.store(client.is_none(), core::sync::atomic::Ordering::Relaxed);
    let mut matrix = right_link::Matrix::new();
    let mut app = Session::new(route::settings());
    app.enable_host_switch();
    app.set_host_target(route::target());
    let mut gate = InputGate::new();
    let mut switch = BrailleSwitch::default();
    let mut base = Base::new();
    let mut braille = false;
    let mut touch_menu = false;
    let mut blocked = 0;
    let mut previous_keys = 0;
    let mut last_model = None;
    loop {
        let sample = matrix.poll(true).await;
        if let Some(previous) = route::take_import() {
            app.settings = previous;
            app.cancel_for_link_change(braille_keys(sample.keys));
        }
        let docked = relay::docked();
        if touch_menu && !docked {
            app.cancel(braille_keys(sample.keys));
            blocked |= sample.keys;
            touch_menu = false;
        }
        let viewport = if docked { (280, 240) } else { (144, 168) };
        app.set_viewport(viewport.0, viewport.1);
        let right_bootloader = right_link::BOOTLOADER
            .available()
            .filter(|_| sample.right_connected);
        let dock_bootloader = relay::BOOTLOADER.available();
        app.set_bootloader_devices(
            BOOTLOADER_LEFT
                | if right_bootloader.is_some() {
                    BOOTLOADER_RIGHT
                } else {
                    0
                }
                | if dock_bootloader.is_some() {
                    BOOTLOADER_PROSPECTOR
                } else {
                    0
                },
        );
        let mut keys = sample.keys;
        blocked &= keys;
        keys &= !blocked;
        let output = route::connection();
        let input = gate.observe(output.generation(), true, keys, sample.invalidated);
        if input.cancel {
            base = Base::new();
            app.cancel_for_link_change(braille_keys(keys));
            if sample.invalidated {
                gate.acknowledge_generation(route::invalidate_pending_reports());
            }
        }
        let next = switch.update(sample.now, input.keys);
        let transition = braille != next;
        let Some(guest) = client.as_mut() else {
            continue;
        };
        let mut final_change = Ok(None);
        if transition {
            if braille && !touch_menu {
                final_change = app
                    .finish_momentary(input.keys.map(braille_keys), sample.now, guest)
                    .map(Some);
            } else if !touch_menu {
                app.cancel(0);
            }
            blocked |= braille_transition_blocked(next, previous_keys, keys);
            keys &= !blocked;
            base = Base::new();
            gate.acknowledge_generation(route::invalidate_pending_reports());
            braille = next;
        }
        previous_keys = keys;
        if input.keys.is_some() || transition {
            let result = if transition && !braille && !touch_menu {
                final_change
            } else if braille || touch_menu {
                app.update(braille_keys(keys), sample.now, guest).map(Some)
            } else {
                base.process(guest, keys, sample.now).await.map(|()| None)
            };
            match result {
                Err(_) => {
                    super::COMPONENT_FAILED.store(true, core::sync::atomic::Ordering::Relaxed);
                    route::invalidate_pending_reports();
                    client.take();
                    continue;
                }
                Ok(Some(change)) => {
                    apply_change(
                        change,
                        &mut app,
                        &mut gate,
                        keys,
                        right_bootloader,
                        dock_bootloader,
                    )
                    .await;
                }
                Ok(None) => {}
            }
        }
        if touch_menu && !app.menu_open() {
            blocked |= keys;
            touch_menu = false;
        }
        let mut model = display_model(&app, &sample, braille || touch_menu);
        if let Some(command) = relay::take_touch(model) {
            let opening = !app.menu_open();
            let Ok(change) = app.navigate_action(command, guest) else {
                super::COMPONENT_FAILED.store(true, core::sync::atomic::Ordering::Relaxed);
                route::invalidate_pending_reports();
                client.take();
                continue;
            };
            if change.reset {
                base = Base::new();
                gate.observe(route::connection().generation(), true, keys, true);
            }
            apply_change(
                change,
                &mut app,
                &mut gate,
                keys,
                right_bootloader,
                dock_bootloader,
            )
            .await;
            if touch_menu && !app.menu_open() {
                blocked |= keys;
            }
            touch_menu = (touch_menu || opening) && app.menu_open();
            model = display_model(&app, &sample, braille || touch_menu);
        }
        if last_model != Some(model) {
            relay::publish(model);
            if let Ok(scene) = guest.render(144, 168, model.bytes.as_slice(), model.detail.as_str())
            {
                if let Some(scene) = kb_display::Scene::from_bytes(scene.as_slice()) {
                    crate::toucan_display::present(scene);
                    last_model = Some(model);
                }
            }
        }
    }
}

fn display_model(app: &Session, sample: &right_link::Sample, open: bool) -> Model {
    let links = Links {
        online: 1
            | (u8::from(sample.right_connected) << 1)
            | (u8::from(route::connection().ready()) << 2)
            | (u8::from(route::target().is_bluetooth()) << 4),
        battery: [crate::battery::level(), sample.right_battery],
        target: route::target(),
        connection: route::status(relay::docked()),
    };
    if open {
        app.model(links)
    } else {
        Model::home(links)
    }
}

async fn apply_change(
    change: Change,
    app: &mut Session,
    gate: &mut InputGate,
    keys: u64,
    right_bootloader: Option<u32>,
    dock_bootloader: Option<u32>,
) {
    if change.bootloader != 0 {
        let generation = route::invalidate_pending_reports();
        gate.observe(generation, true, keys, true);
        app.cancel_for_link_change(braille_keys(keys));
        let accepted =
            enter_bootloaders(change.bootloader, right_bootloader, dock_bootloader).await;
        app.bootloader_result(change.bootloader, accepted);
    }
    if let Some(target) = change.target {
        if !route::switch(target, &mut app.settings).await {
            app.busy();
        }
        app.set_host_target(route::target());
    }
    if change.reset {
        gate.acknowledge_generation(route::invalidate_pending_reports());
    }
    if change.save && !route::save(&app.settings) {
        app.busy();
    }
    if let Some(effect) = change.bluetooth {
        let control = match effect {
            BluetoothAction::Reconnect => crate::settings::BluetoothControl::Reconnect,
            BluetoothAction::ClearBonds => crate::settings::BluetoothControl::ClearBonds,
        };
        if !crate::settings::request_bluetooth(control) {
            app.busy();
        }
    }
    if let Some(token) = route::connection().token() {
        for stroke in change.strokes.as_slice() {
            if !route::send(token, Output::Stroke(*stroke)).await {
                break;
            }
        }
    }
}

async fn enter_bootloaders(requested: u8, right: Option<u32>, dock: Option<u32>) -> u8 {
    let (right, dock) = embassy_futures::join::join(
        right_link::BOOTLOADER.request(right.filter(|_| requested & BOOTLOADER_RIGHT != 0)),
        relay::BOOTLOADER.request(dock.filter(|_| requested & BOOTLOADER_PROSPECTOR != 0)),
    )
    .await;
    let accepted =
        if right { BOOTLOADER_RIGHT } else { 0 } | if dock { BOOTLOADER_PROSPECTOR } else { 0 };
    if requested & BOOTLOADER_LEFT != 0 && accepted == requested & !BOOTLOADER_LEFT {
        crate::bootloader::enter_uf2().await;
    }
    accepted
}
