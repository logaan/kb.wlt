use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_time::Instant;
use kb_component_runtime::{BrailleComponent, BrailleTyping};
use kb_core::{
    KeyStroke,
    preferences::{MAX_OUTPUT, Settings},
    transport::{FaultLatch, InputGate},
};
pub static OUTPUT: Channel<CriticalSectionRawMutex, (u32, crate::usb_hid::Output), 64> =
    Channel::new();
pub async fn run(mut next: impl AsyncFnMut(bool) -> Option<(kb_core::split::Update, u8)>) {
    const PENDING_CAPACITY: usize = 128;
    const BATCH_CAPACITY: usize = MAX_OUTPUT;
    let mut component = FaultLatch::new(BrailleComponent::new(include_bytes!(env!(
        "KB_BRAILLE_WASM"
    ))));
    let mut fault_seen = false;
    let mut settings = crate::settings::current()
        .and_then(|blob| Settings::decode(blob.as_slice()).ok())
        .unwrap_or_default();
    let mut pending = kb_core::pipeline::Bounded::<KeyStroke, PENDING_CAPACITY>::default();
    let mut typing = BrailleTyping::new(true);
    let base_catalog = {
        let mut catalog = kb_core::reference::Catalog::new();
        let mut failed = false;
        for cell in 1..=511 {
            let result = component.call(|component| {
                let packed = component.map_cell(cell)?;
                catalog
                    .append(cell, packed)
                    .map_err(|_| kb_component_runtime::Error::InvalidReturn)
            });
            if result.is_err() {
                failed = true;
                break;
            }
            embassy_futures::yield_now().await;
        }
        if failed { None } else { Some(catalog) }
    };
    let mut reference = kb_core::reference::State::new();
    let mut catalog = base_catalog
        .as_ref()
        .map(|base| base.for_settings(&settings));
    let mut menu = kb_core::menu::State::default();
    let mut menu_page = None;
    let mut cursor = kb_core::reference::Cursor::default();
    let mut reference_page = None;
    let mut input = InputGate::new();
    let mut display_status = kb_display::Status {
        transport: kb_display::Transport::Split,
        ..Default::default()
    };
    loop {
        let accept = if cfg!(feature = "toucan-dongle") {
            pending.is_empty()
        } else {
            PENDING_CAPACITY - pending.len() >= BATCH_CAPACITY
        };
        let Some((update, peers)) = next(accept).await else {
            return;
        };
        let now = Instant::now().as_millis() as u32;
        let session = crate::usb_hid::connection();
        let mut generation = session.generation();
        let input_update = input.observe(
            generation,
            true,
            u64::from(update.keys),
            update.cancel_chord,
        );
        if input_update.cancel {
            OUTPUT.clear();
            pending = Default::default();
            if update.cancel_chord {
                generation = crate::usb_hid::invalidate_pending_reports();
                input.acknowledge_generation(generation);
            }

            reference = reference.cancel(update.keys).state;
            menu = Default::default();
            typing.cancel();
        }

        let forward = if input_update.keys.is_some() {
            let previous_mode = reference.mode;
            let transition = reference.step(update.keys, now);
            reference = transition.state;
            let editor_key = match transition.action {
                kb_core::reference::Action::Lookup(cell) => kb_core::computer8(cell),
                _ => None,
            };
            let menu_update = menu.step(reference.mode, transition.action, &settings, editor_key);
            menu = menu_update.state;
            reference.mode = menu_update.mode;
            let mut settings_changed = false;
            match menu_update.effect {
                kb_core::menu::Effect::None => {}
                kb_core::menu::Effect::SetMode(mode) => {
                    settings.mode = mode;
                    settings_changed = true;
                }
                kb_core::menu::Effect::SetBinding { slot, binding } => {
                    settings_changed = settings
                        .set_binding(usize::from(slot), binding.trigger(), binding.text())
                        .is_ok();
                }
                kb_core::menu::Effect::ClearBinding(slot) => {
                    settings_changed = settings.clear_binding(usize::from(slot)).is_ok();
                }
                kb_core::menu::Effect::Bluetooth(action) => {
                    let command = match action {
                        kb_core::menu::BluetoothAction::Reconnect => {
                            crate::settings::BluetoothControl::Reconnect
                        }
                        kb_core::menu::BluetoothAction::ClearBonds => {
                            crate::settings::BluetoothControl::ClearBonds
                        }
                    };
                    if !crate::settings::request_bluetooth(command) {
                        menu.notice = kb_core::menu::Notice::Busy;
                    }
                }
            }
            if settings_changed {
                typing.cancel();
                let mut bytes = [0; kb_core::preferences::ENCODED_LEN];
                if settings.encode(&mut bytes).is_err() || !crate::settings::request_save(&bytes) {
                    menu.notice = kb_core::menu::Notice::Busy;
                }
                catalog = base_catalog
                    .as_ref()
                    .map(|base| base.for_settings(&settings));
                cursor = Default::default();
            }
            if reference.mode == kb_core::reference::Mode::Guide
                && settings.mode != kb_core::preferences::BrailleMode::Ueb1
            {
                reference.mode = kb_core::reference::Mode::Reference;
            }
            if reference.mode == kb_core::reference::Mode::Guide {
                cursor = cursor.navigate(transition.action, kb_core::ueb1::RULES.len() as u16);
            } else if let Some(catalog) = catalog.as_ref() {
                if previous_mode != reference.mode
                    && reference.mode == kb_core::reference::Mode::Reference
                {
                    cursor = Default::default();
                }
                cursor = cursor.apply(transition.action, catalog);
            } else if matches!(
                reference.mode,
                kb_core::reference::Mode::Reference | kb_core::reference::Mode::Find
            ) {
                reference.mode = kb_core::reference::Mode::Diagnostics;
            }
            if transition.reset_braille && previous_mode == kb_core::reference::Mode::Typing {
                typing.cancel();
                pending = Default::default();
                OUTPUT.clear();
                generation = crate::usb_hid::invalidate_pending_reports();
                input.acknowledge_generation(generation);
            }
            transition.forward
        } else {
            false
        };
        if session.ready()
            && input_update.keys.is_some()
            && forward
            && !component.is_faulted()
            && PENDING_CAPACITY - pending.len() >= BATCH_CAPACITY
        {
            let output_batch = typing.step(&mut component, &settings, update.keys, now);
            pending.extend_from_slice(output_batch.as_slice()).unwrap();
        }
        if !fault_seen && component.is_faulted() {
            fault_seen = true;
            typing.cancel();
            pending = Default::default();
            OUTPUT.clear();
            generation = crate::usb_hid::invalidate_pending_reports();
            input.acknowledge_generation(generation);

            reference = reference.cancel(update.keys).state;
        }
        let mut sent = 0;
        for stroke in pending.as_slice() {
            if OUTPUT
                .try_send((generation, crate::usb_hid::Output::Stroke(*stroke)))
                .is_err()
            {
                break;
            }
            sent += 1;

            display_status.last_usage = stroke.usage;
            display_status.last_modifiers = stroke.modifiers;
        }
        if sent != 0 {
            pending = kb_core::pipeline::Bounded::from_slice(
                &pending.as_slice()[sent..],
                KeyStroke::default(),
            )
            .unwrap();
        }

        {
            let connected = session.ready()
                && peers
                    >= if cfg!(feature = "toucan-dongle") {
                        2
                    } else {
                        1
                    };
            let pairing_unsaved = crate::ble_bonds::storage_failed();
            let previous_live = display_status.braille;
            let changed = sent != 0
                || display_status.pairing_unsaved != pairing_unsaved
                || display_status.faulted != component.is_faulted()
                || display_status.connected != connected
                || display_status.held != typing.held()
                || display_status.chord != typing.chord()
                || update.pressed | update.released != 0;
            display_status.connected = connected;
            display_status.faulted = component.is_faulted();
            display_status.pairing_unsaved = pairing_unsaved;
            display_status.held = typing.held();
            display_status.chord = typing.chord();
            #[cfg(feature = "toucan-dongle")]
            {
                let keyboard = kb_display::KeyboardStatus {
                    active_keys: update.keys.count_ones() as u8,
                    layer: kb_display::keyboard::BRAILLE_LAYER,
                };
                let chord = typing.chord();
                let release = if chord == 0 {
                    Default::default()
                } else {
                    let mapped = base_catalog.as_ref().and_then(|base| {
                        base.find(chord)
                            .map(|i| base.entries()[i as usize].stroke())
                    });
                    let (result, waiting) = typing.preview_release(&settings, mapped, now);
                    kb_display::keyboard::release_label(
                        result.as_slice(),
                        waiting,
                        chord == kb_core::reference::LEADER,
                    )
                };
                display_status.braille = Some(kb_display::keyboard::LiveBraille {
                    mode: settings.mode,
                    buffered: typing.buffered_cells(),
                    release,
                });
                display_status.keyboard = Some(keyboard);
                crate::toucan::receiver::publish_feedback(display_status, keyboard, now);
            }
            if matches!(
                reference.mode,
                kb_core::reference::Mode::Namespace
                    | kb_core::reference::Mode::Reference
                    | kb_core::reference::Mode::Find
                    | kb_core::reference::Mode::Guide
            ) {
                let page = catalog.as_ref().map_or(
                    kb_core::reference::Page {
                        mode: reference.mode,
                        braille_mode: settings.mode,
                        ..Default::default()
                    },
                    |catalog| cursor.active_page(catalog, reference.mode, &settings),
                );
                menu_page = None;
                if reference_page != Some(page) {
                    reference_page = Some(page);
                    crate::prospector::show_reference(page);
                }
            } else if !matches!(
                reference.mode,
                kb_core::reference::Mode::Typing | kb_core::reference::Mode::ReleaseGuard
            ) {
                let diagnostics = reference.mode == kb_core::reference::Mode::Diagnostics;
                let page = kb_display::menu::Page {
                    mode: reference.mode,
                    state: menu,
                    settings,
                    diagnostics: kb_core::menu::Diagnostics {
                        uptime_seconds: if diagnostics {
                            (Instant::now().as_secs()) as u32
                        } else {
                            0
                        },
                        usb_ready: session.ready(),
                        peers: peers,
                        held: if diagnostics { update.keys } else { 0 },
                        last_usage: display_status.last_usage,
                        last_modifiers: display_status.last_modifiers,
                        component_fault: component.is_faulted(),
                        pairing_unsaved,
                        settings_unsaved: crate::settings::unsaved(),
                    },
                };
                reference_page = None;
                if menu_page != Some(page) {
                    menu_page = Some(page);
                    crate::prospector::show_menu(page);
                }
            } else {
                let leaving_menu = menu_page.take().is_some() | reference_page.take().is_some();
                if changed || leaving_menu || display_status.braille != previous_live {
                    crate::prospector::update(display_status);
                }
            }
        }
    }
}
