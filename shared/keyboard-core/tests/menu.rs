use kb_core::{
    KeyStroke, computer8,
    menu::{BluetoothAction, Effect, Notice, State},
    preferences::{BrailleMode, Settings},
    reference::{Action, Mode, State as Capture},
    transport::{InputGate, SessionGate},
};

#[test]
fn namespace_discovers_each_screen_with_space_held_or_released() {
    for (cell, mode) in [
        (0x11f, Mode::Main),
        (0x117, Mode::Reference),
        (0x109, Mode::Config),
        (0x103, Mode::Bindings),
        (0x119, Mode::Diagnostics),
    ] {
        for held in [false, true] {
            let mut state = Capture::new();
            for keys in [
                0x11f,
                if held { 0x100 } else { 0 },
                cell,
                if held { 0x100 } else { 0 },
            ] {
                let next = state.step(keys, 0);
                if next.state.mode != Mode::Typing {
                    assert!(!next.forward);
                }
                state = next.state;
            }
            assert_eq!(state.mode, mode);
        }
    }
}

#[test]
fn menu_navigation_changes_modes_without_host_output() {
    let settings = Settings::default();
    let state = State::default()
        .step(Mode::Main, Action::Next, &settings, None)
        .state;
    assert_eq!(
        state
            .step(Mode::Main, Action::Activate, &settings, None)
            .mode,
        Mode::Config
    );
    let update = State::default().step(Mode::Config, Action::Activate, &settings, None);
    assert_eq!(update.effect, Effect::SetMode(BrailleMode::Ueb1));
    assert_eq!(
        State::default()
            .step(Mode::Main, Action::Back, &settings, None)
            .mode,
        Mode::ReleaseGuard
    );
}

#[test]
fn editor_captures_exact_trigger_count_then_saves_text() {
    let settings = Settings::default();
    let mut state = State::default()
        .step(Mode::Bindings, Action::Activate, &settings, None)
        .state;
    state.selected = 1;
    let update = state.step(Mode::BindingOptions, Action::Activate, &settings, None);
    assert_eq!(update.mode, Mode::BindingCapture);
    let update = update
        .state
        .step(update.mode, Action::Lookup(0x140), &settings, None);
    assert_eq!(update.mode, Mode::BindingCapture);
    let update = update
        .state
        .step(update.mode, Action::Lookup(0x103), &settings, None);
    assert_eq!(update.mode, Mode::BindingText);
    let update = update.state.step(
        update.mode,
        Action::Lookup(1),
        &settings,
        KeyStroke::from_ascii(b'A'),
    );
    let update = update.state.step(
        update.mode,
        Action::Lookup(1),
        &settings,
        KeyStroke::from_ascii(b'b'),
    );
    let update = update.state.step(
        update.mode,
        Action::Lookup(0x40),
        &settings,
        KeyStroke::from_ascii(8),
    );
    let update = update
        .state
        .step(update.mode, Action::Activate, &settings, None);
    let Effect::SetBinding { slot, binding } = update.effect else {
        panic!("binding not saved")
    };
    assert_eq!(slot, 0);
    assert_eq!(binding.trigger(), [0x140, 0x103]);
    assert_eq!(binding.text(), b"A");
    assert_eq!(update.mode, Mode::Bindings);
}

#[test]
fn editor_validates_empty_duplicate_and_bounded_text_and_can_cancel() {
    let mut settings = Settings::default();
    settings.set_binding(1, &[1], b"old").unwrap();
    let mut state = State::default();
    state.draft.trigger[0] = 1;
    state.draft.trigger_len = 1;
    let update = state.step(Mode::BindingText, Action::Activate, &settings, None);
    assert_eq!(update.state.notice, Notice::EmptyText);
    let mut state = update.state;
    for _ in 0..25 {
        state = state
            .step(
                Mode::BindingText,
                Action::Lookup(1),
                &settings,
                KeyStroke::from_ascii(b'a'),
            )
            .state;
    }
    assert_eq!(state.draft.text_len, 24);
    assert_eq!(state.notice, Notice::TextFull);
    let update = state.step(Mode::BindingText, Action::Activate, &settings, None);
    assert_eq!(update.state.notice, Notice::DuplicateTrigger);
    assert_eq!(update.effect, Effect::None);
    assert_eq!(
        update
            .state
            .step(Mode::BindingText, Action::Back, &settings, None)
            .mode,
        Mode::Bindings
    );
}

#[test]
fn deleting_a_slot_is_an_explicit_action() {
    let settings = Settings::default();
    let mut state = State::default();
    state.slot = 8;
    state.selected = 5;
    let update = state.step(Mode::BindingOptions, Action::Activate, &settings, None);
    assert_eq!(update.effect, Effect::ClearBinding(8));
    assert_eq!(update.mode, Mode::Bindings);
}

#[test]
fn editor_updates_existing_expansion_without_recapturing_trigger() {
    let mut settings = Settings::default();
    settings.set_binding(3, &[0x140, 1], b"old").unwrap();
    let state = State {
        slot: 3,
        selected: 4,
        ..State::default()
    };
    let update = state.step(Mode::BindingOptions, Action::Activate, &settings, None);
    assert_eq!(update.mode, Mode::BindingText);
    assert_eq!(update.state.draft, settings.bindings[3]);
    let update = update.state.step(
        update.mode,
        Action::Lookup(0x40),
        &settings,
        computer8(0x40),
    );
    let update = update
        .state
        .step(update.mode, Action::Activate, &settings, None);
    let Effect::SetBinding { slot, binding } = update.effect else {
        panic!("binding not saved")
    };
    assert_eq!(slot, 3);
    assert_eq!(binding.trigger(), [0x140, 1]);
    assert_eq!(binding.text(), b"ol");
}

#[test]
fn config_dispatches_each_bluetooth_action() {
    let settings = Settings::default();
    for (selected, expected) in [
        (1, BluetoothAction::Reconnect),
        (2, BluetoothAction::ClearBonds),
    ] {
        let state = State {
            selected,
            ..State::default()
        };
        let update = state.step(Mode::Config, Action::Activate, &settings, None);
        assert_eq!(update.effect, Effect::Bluetooth(expected));
        assert_eq!(update.mode, Mode::Config);
    }
}

#[test]
fn menu_and_editor_chords_remain_captured_through_held_space_exit() {
    let settings = Settings::default();
    let mut state = State::default();
    let mut capture = Capture::new().step(0x11f, 0).state;
    let mut step = |keys, time| {
        let transition = capture.step(keys, time);
        assert!(!transition.forward);
        let editor_key = match transition.action {
            Action::Lookup(cell) => computer8(cell),
            _ => None,
        };
        let update = state.step(
            transition.state.mode,
            transition.action,
            &settings,
            editor_key,
        );
        capture = transition.state;
        capture.mode = update.mode;
        state = update.state;
        update
    };
    step(0x100, 1);
    step(0x103, 2);
    assert_eq!(step(0x100, 3).mode, Mode::Bindings);
    step(0x140, 4);
    assert_eq!(step(0x100, 5).mode, Mode::BindingOptions);
    step(0x140, 6);
    assert_eq!(step(0x100, 7).mode, Mode::BindingCapture);
    step(0x140, 8);
    assert_eq!(step(0x100, 9).mode, Mode::BindingText);
    assert_eq!(step(0, 10).state.draft.text(), b"");
    step(1, 11);
    assert_eq!(step(0, 12).state.draft.text(), b"a");
    step(0x140, 13);
    assert!(matches!(step(0x100, 14).effect, Effect::SetBinding { .. }));
    step(0x103, 15);
    assert_eq!(step(0x100, 16).mode, Mode::Main);
    step(0x103, 17);
    assert_eq!(step(0x100, 18).mode, Mode::ReleaseGuard);
    assert_eq!(step(0x101, 19).mode, Mode::ReleaseGuard);
    assert_eq!(step(0, 20).mode, Mode::Typing);
    assert!(capture.step(1, 21).forward);
}

#[test]
fn invalidating_queued_reports_preserves_the_held_space_namespace() {
    let mut usb = SessionGate::new().set_ready(true);
    let mut gate = InputGate::new();
    let mut capture = Capture::new();
    let settings = Settings::default();
    let mut menu = State::default();
    assert!(gate.observe(usb.generation(), true, 0, false).cancel);
    assert_eq!(gate.observe(usb.generation(), true, 0, false).keys, Some(0));

    for (time, keys) in [0x11f, 0x100, 0x11f, 0x100, 0x108, 0x100]
        .into_iter()
        .enumerate()
    {
        let input = gate.observe(usb.generation(), true, keys, false);
        assert!(!input.cancel);
        let transition = capture.step(input.keys.unwrap() as u16, time as u32);
        if capture.mode == Mode::Typing && transition.reset_braille {
            let old_token = usb.token().unwrap();
            usb = usb.invalidate();
            gate.acknowledge_generation(usb.generation());
            assert!(!usb.accepts(old_token));
        }
        let update = menu.step(transition.state.mode, transition.action, &settings, None);
        capture = transition.state;
        capture.mode = update.mode;
        menu = update.state;
    }

    assert_eq!(capture.mode, Mode::Main);
    assert_eq!(menu.selected, 1);
    usb = usb.set_ready(false);
    assert!(gate.observe(usb.generation(), true, 0x100, false).cancel);
}
