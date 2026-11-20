use super::{relay, right_link, route};
use crate::usb_hid::Output;
use kb_client_session::{Links, Model, Session};
use kb_component_runtime::{DisplayGuest, Error, KeyboardComponent};
use kb_core::{
    matrix_link::{BrailleSwitch, RouteSwitch, braille_keys, braille_transition_blocked},
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
    let mut gate = InputGate::new();
    let mut switch = BrailleSwitch::default();
    let mut route_switch = RouteSwitch::new();
    let mut base = Base::new();
    let mut braille = false;
    let mut blocked = 0;
    let mut previous_keys = 0;
    let mut last_model = None;
    loop {
        let sample = matrix.poll(true).await;
        if let Some(previous) = route::take_import() {
            app = Session::new(previous);
            app.enable_host_switch();
            app.cancel(braille_keys(sample.keys));
        }
        let viewport = if relay::docked() {
            (280, 240)
        } else {
            (144, 168)
        };
        app.set_viewport(viewport.0, viewport.1);
        let mut keys = sample.keys;
        if route_switch.update(sample.now, keys) && !route::switch(&mut app.settings) {
            app.busy();
        }
        let routing = route_switch.blocked();
        if routing {
            blocked |= keys;
        }
        blocked &= keys;
        keys &= !blocked;
        let output = route::connection();
        let input = gate.observe(output.generation(), true, keys, sample.invalidated);
        if input.cancel {
            base = Base::new();
            app.cancel(braille_keys(keys));
            if sample.invalidated {
                gate.acknowledge_generation(route::invalidate_pending_reports());
            }
        }
        let next = switch.update(sample.now, if routing { None } else { input.keys });
        let transition = braille != next;
        let Some(guest) = client.as_mut() else {
            continue;
        };
        let mut final_change = Ok(None);
        if transition {
            if braille {
                let final_keys = if routing {
                    None
                } else {
                    input.keys.map(braille_keys)
                };
                final_change = app
                    .finish_momentary(final_keys, sample.now, guest)
                    .map(Some);
            } else {
                app.cancel(0);
            }
            braille = next;
            blocked |= braille_transition_blocked(braille, previous_keys, keys);
            keys &= !blocked;
            base = Base::new();
            gate.acknowledge_generation(route::invalidate_pending_reports());
        }
        previous_keys = keys;
        if input.keys.is_some() || transition {
            let result = if transition && !braille {
                final_change
            } else if braille {
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
                    if change.reset {
                        gate.acknowledge_generation(route::invalidate_pending_reports());
                    }
                    if change.save && !route::save(&app.settings) {
                        app.busy();
                    }
                    if let Some(effect) = change.bluetooth {
                        let control = match effect {
                            BluetoothAction::Reconnect => {
                                crate::settings::BluetoothControl::Reconnect
                            }
                            BluetoothAction::ClearBonds => {
                                crate::settings::BluetoothControl::ClearBonds
                            }
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
                Ok(None) => {}
            }
        }
        let links = Links {
            online: 1
                | (u8::from(sample.right_connected) << 1)
                | (u8::from(output.ready()) << 2)
                | (u8::from(route::direct()) << 4),
            battery: [crate::battery::level(), sample.right_battery],
        };
        let model = if braille {
            app.model(links)
        } else {
            Model::home(links)
        };
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
