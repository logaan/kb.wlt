use kb_client_session::{
    BOOTLOADER_LEFT, BOOTLOADER_PROSPECTOR, BOOTLOADER_RIGHT, Change, Guest, Links, Model, Session,
    Viewport,
};
use kb_component_runtime::{BrailleComponent, DisplayGuest, KeyboardComponent};
use kb_core::{
    KeyStroke,
    pipeline::{Action, Input, Key, KeyEvent, KeyState},
    preferences::{BrailleMode, Settings},
    reference::{LEADER, SPACE},
};

const TOUCAN: &[u8] =
    include_bytes!("../../../clients/components/generated/toucan-keymap/keyboard.core.wasm");
const PAINTBRUSH: &[u8] = include_bytes!("../../../clients/components/generated/braille.core.wasm");

fn commands(mut scene: &[u8]) -> Vec<&[u8]> {
    let mut result = Vec::new();
    while let Some(opcode) = scene.first() {
        let len = match opcode {
            1 | 2 => 11,
            4 => 12,
            3 => 8 + usize::from(scene[7]),
            _ => panic!("unexpected scene command {opcode}"),
        };
        let (command, rest) = scene.split_at(len);
        result.push(command);
        scene = rest;
    }
    result
}

fn labels(scene: &[u8]) -> Vec<&str> {
    commands(scene)
        .into_iter()
        .filter(|command| command[0] == 3)
        .map(|command| std::str::from_utf8(&command[8..]).unwrap())
        .collect()
}

fn masks(scene: &[u8]) -> Vec<(u8, bool)> {
    commands(scene)
        .into_iter()
        .filter(|command| command[0] == 4)
        .map(|command| (command[9], command[10] & 0x80 != 0))
        .collect()
}

fn key(session: &mut Session, guest: &mut impl DisplayGuest, keys: u16) -> Change {
    session.update(keys, 10, guest).unwrap()
}

fn tap(session: &mut Session, guest: &mut impl DisplayGuest, cell: u16) -> Change {
    assert!(key(session, guest, cell).strokes.is_empty());
    key(session, guest, 0)
}

fn screen(session: &Session) -> (u8, u16) {
    let model = session.model(Links::default());
    let bytes = model.bytes.as_slice();
    (bytes[0], u16::from_le_bytes([bytes[1], bytes[2]]))
}

fn base_press(guest: &KeyboardComponent) {
    let input = Input::<1>::with_events(
        0,
        &[KeyEvent {
            position: 13,
            state: KeyState::Pressed,
            timestamp_ms: 0,
        }],
    )
    .unwrap();
    let result = guest.process::<42, 1, 1>(&[], &input).unwrap();
    assert_eq!(
        result.actions.as_slice(),
        &[Action::Press(Key {
            usage: 4,
            modifiers: 0,
        })],
    );
}

#[test]
fn debug_requests_only_the_selected_available_devices_on_activation() {
    let mut guest = KeyboardComponent::new(TOUCAN).unwrap();
    let mut session = Session::new(Settings::default());
    session.set_bootloader_devices(BOOTLOADER_LEFT | BOOTLOADER_RIGHT | BOOTLOADER_PROSPECTOR);
    for cell in [LEADER, SPACE | 25, SPACE | 64] {
        assert_eq!(tap(&mut session, &mut guest, cell).bootloader, 0);
    }
    assert_eq!(screen(&session), (14, 0));
    for cell in [SPACE | 64, SPACE | 8, SPACE | 8, SPACE | 64, SPACE | 8] {
        assert_eq!(tap(&mut session, &mut guest, cell).bootloader, 0);
    }
    let requested = BOOTLOADER_LEFT | BOOTLOADER_PROSPECTOR;
    let model = session.model(Links::default());
    assert_eq!(model.bytes.as_slice()[16], requested);
    let wide = guest.render(280, 240, model.bytes.as_slice(), "").unwrap();
    let labels = labels(wide.as_slice());
    assert!(
        labels
            .windows(2)
            .any(|line| line == ["Left half", "Selected"])
    );
    assert!(
        labels
            .windows(2)
            .any(|line| line == ["Right half", "Ready"])
    );
    assert!(
        labels
            .windows(2)
            .any(|line| line == ["Prospector", "Selected"])
    );
    let narrow = guest.render(144, 168, model.bytes.as_slice(), "").unwrap();
    let narrow = commands(narrow.as_slice());
    assert_eq!(
        narrow
            .iter()
            .filter(|cmd| cmd[0] == 2 && cmd[1] == 122 && cmd[5] == 16)
            .count(),
        3
    );
    assert_eq!(
        narrow
            .iter()
            .filter(|cmd| cmd[0] == 1 && cmd[1] == 126 && cmd[5] == 8)
            .count(),
        2
    );
    assert_eq!(
        tap(&mut session, &mut guest, SPACE | 64).bootloader,
        requested
    );
    session.bootloader_result(requested, BOOTLOADER_PROSPECTOR);
    session.set_bootloader_devices(BOOTLOADER_LEFT | BOOTLOADER_RIGHT);
    session.cancel_for_link_change(0);
    assert_eq!(screen(&session), (14, 3));
    let model = session.model(Links::default());
    assert_eq!(model.bytes.as_slice()[15..18], [3, BOOTLOADER_LEFT, 4]);
    assert_eq!(tap(&mut session, &mut guest, SPACE | 3).bootloader, 0);
    assert_eq!(screen(&session), (9, 0));
    assert_eq!(tap(&mut session, &mut guest, SPACE | 64).bootloader, 0);
    assert_eq!(session.model(Links::default()).bytes.as_slice()[16], 0);
}

#[test]
fn debug_ignores_unavailable_devices_and_empty_requests() {
    let mut guest = BrailleComponent::new(PAINTBRUSH).unwrap();
    let mut session = Session::new(Settings::default());
    session.set_bootloader_devices(BOOTLOADER_PROSPECTOR);
    for cell in [LEADER, SPACE | 25, SPACE | 64, SPACE | 64] {
        assert_eq!(tap(&mut session, &mut guest, cell).bootloader, 0);
    }
    assert_eq!(
        session.model(Links::default()).bytes.as_slice()[15..18],
        [4, 0, 2]
    );
    for cell in [SPACE | 8, SPACE | 8, SPACE | 8, SPACE | 64] {
        assert_eq!(tap(&mut session, &mut guest, cell).bootloader, 0);
    }
    assert_eq!(session.model(Links::default()).bytes.as_slice()[17], 1);
    tap(&mut session, &mut guest, SPACE | 1);
    tap(&mut session, &mut guest, SPACE | 64);
    tap(&mut session, &mut guest, SPACE | 8);
    assert_eq!(
        tap(&mut session, &mut guest, SPACE | 64).bootloader,
        BOOTLOADER_PROSPECTOR
    );
}

#[test]
fn both_production_clients_render_battery_gauges_and_wireless_status() {
    let model = Model::home(Links {
        online: 0b101,
        battery: [85, 10],
    });
    let mut toucan = KeyboardComponent::new(TOUCAN).unwrap();
    let mut paintbrush = BrailleComponent::new(PAINTBRUSH).unwrap();
    for guest in [&mut toucan as &mut dyn DisplayGuest, &mut paintbrush] {
        for (width, height) in [(144, 168), (280, 240)] {
            let scene = guest
                .render(width, height, model.bytes.as_slice(), "")
                .unwrap();
            assert_eq!(labels(scene.as_slice()), ["85%", "Offline"]);
            let scene = commands(scene.as_slice());
            assert!(
                scene
                    .iter()
                    .any(|command| command[0] == 2 && command[9] == 5)
            );
            assert!(
                scene
                    .iter()
                    .any(|command| command[0] == 2 && command[9] == 6)
            );
            for direct in [0, 0x10] {
                let online = Model::home(Links {
                    online: 0b111 | direct,
                    battery: [255, 100],
                });
                let online = guest
                    .render(width, height, online.bytes.as_slice(), "")
                    .unwrap();
                assert_eq!(labels(online.as_slice()), ["?", "100%"]);
                let off = Model::home(Links {
                    online: direct,
                    battery: [255; 2],
                });
                let off = guest
                    .render(width, height, off.bytes.as_slice(), "")
                    .unwrap();
                assert_eq!(labels(off.as_slice()), ["Offline", "Offline"]);
                assert_ne!(online.as_slice(), off.as_slice());
            }
        }
    }
}

#[test]
fn toucan_previews_and_emits_braille_in_its_normal_keyboard_instance() {
    let mut guest = KeyboardComponent::new(TOUCAN).unwrap();
    let mut session = Session::new(Settings::default());
    base_press(&guest);
    let idle = session.model(Links::default());
    let idle = guest
        .render(280, 240, idle.bytes.as_slice(), idle.detail.as_str())
        .unwrap();
    assert_eq!(masks(idle.as_slice()), [(0, false)]);
    assert!(labels(idle.as_slice()).is_empty());

    assert!(key(&mut session, &mut guest, 19).strokes.is_empty());
    let held = session.model(Links::default());
    let held = guest
        .render(280, 240, held.bytes.as_slice(), held.detail.as_str())
        .unwrap();
    assert_eq!(masks(held.as_slice()), [(13, false)]);
    assert_eq!(labels(held.as_slice()), ["h"]);
    assert_eq!(
        key(&mut session, &mut guest, 0).strokes.as_slice(),
        &[KeyStroke::from_ascii(b'h').unwrap()],
    );
    let settled = session.model(Links::default());
    assert!(settled.detail.as_str().is_empty());
    base_press(&guest);
}

#[test]
fn held_space_navigation_help_and_editing_use_real_chords() {
    let mut guest = KeyboardComponent::new(TOUCAN).unwrap();
    let mut session = Session::new(Settings::default());
    for (keys, expected) in [
        (LEADER, (1, 0)),
        (SPACE, (2, 0)),
        (SPACE | 32, (2, 0)),
        (SPACE, (2, 1)),
        (SPACE | 17, (2, 1)),
        (SPACE, (7, 0)),
        (0, (7, 0)),
    ] {
        assert!(key(&mut session, &mut guest, keys).strokes.is_empty());
        assert_eq!(screen(&session), expected);
    }
    assert!(tap(&mut session, &mut guest, 19).strokes.is_empty());
    assert_eq!(screen(&session), (13, 0));
    tap(&mut session, &mut guest, SPACE | 3);
    assert_eq!(screen(&session), (7, 0));
    assert!(tap(&mut session, &mut guest, SPACE | 17).save);
    assert_eq!(session.settings.mode, BrailleMode::Ueb1);

    tap(&mut session, &mut guest, SPACE | 3);
    assert_eq!(screen(&session), (6, 1));
    tap(&mut session, &mut guest, 3);
    assert_eq!(screen(&session), (8, 0));
    tap(&mut session, &mut guest, SPACE | 17);
    assert_eq!(screen(&session), (10, 0));
    tap(&mut session, &mut guest, SPACE | 17);
    assert_eq!(screen(&session).0, 11);
    tap(&mut session, &mut guest, SPACE | 19);
    assert_eq!(screen(&session), (13, 0));
    tap(&mut session, &mut guest, SPACE | 3);
    assert_eq!(screen(&session).0, 11);
    tap(&mut session, &mut guest, 19);
    assert_eq!(screen(&session).0, 12);
    tap(&mut session, &mut guest, 45);
    assert!(tap(&mut session, &mut guest, SPACE | 17).save);
    assert_eq!(screen(&session), (8, 0));
    assert_eq!(session.settings.bindings[0].trigger(), &[19]);
    assert_eq!(session.settings.bindings[0].text(), b"x");
}

#[test]
fn reference_fills_responsive_pairs_and_preserves_navigation() {
    let mut guest = BrailleComponent::new(PAINTBRUSH).unwrap();
    for (width, height) in [(144, 168), (280, 240)] {
        let page = guest.render(width, height, &[4, 0, 1], "").unwrap();
        let labels = labels(page.as_slice());
        assert_eq!(labels[0], "Capitals");
        let visible = if width == 280 { 16 } else { 12 };
        let letters = [
            "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P",
        ];
        assert_eq!(labels[1..=visible], letters[..visible]);
        assert_eq!(labels[visible + 1], "(H)elp (");
        assert_eq!(masks(page.as_slice()).last(), Some(&(13, false)));

        let nav_reference = guest.render(width, height, &[4, 0, 4], "").unwrap();
        assert_eq!(masks(nav_reference.as_slice())[0], (1, true));
    }
    let transition = |value: u32| (value as u8, (value >> 8) as u16, (value >> 24) as u8);
    for (cell, expected) in [
        (SPACE | 1, (4, 256, 0)),
        (SPACE | 8, (4, 257, 0)),
        (SPACE | 32, (4, 260, 0)),
        (SPACE | 56, (4, 281, 0)),
        (SPACE | 57, (4, 272, 0)),
        (SPACE | 17, (5, 256, 0)),
        (SPACE | 3, (3, 1, 0)),
        (53, (4, 281, 0)),
        (19, (13, 0, 4)),
        (29, (4, 512, 0)),
    ] {
        assert_eq!(
            transition(
                guest
                    .navigate(
                        4,
                        256,
                        cell,
                        Viewport {
                            width: 280,
                            height: 240,
                            host_switch: false
                        }
                    )
                    .unwrap()
            ),
            expected,
        );
    }
}

#[test]
fn page_moves_track_the_viewport_and_help_explains_navigation() {
    let mut guest = KeyboardComponent::new(TOUCAN).unwrap();
    let mut session = Session::new(Settings::default());
    session.set_viewport(144, 168);
    tap(&mut session, &mut guest, LEADER);
    tap(&mut session, &mut guest, 3);
    assert_eq!(screen(&session), (8, 0));
    tap(&mut session, &mut guest, SPACE | 57);
    assert_eq!(screen(&session), (8, 2));
    session.set_viewport(280, 240);
    tap(&mut session, &mut guest, SPACE | 57);
    assert_eq!(screen(&session), (8, 5));
    tap(&mut session, &mut guest, 19);
    for _ in 0..2 {
        tap(&mut session, &mut guest, 29);
    }
    let help = session.model(Links::default());
    let help = guest.render(280, 240, help.bytes.as_slice(), "").unwrap();
    let labels = labels(help.as_slice());
    assert!(labels.contains(&"Return opens"));
    assert!(labels.contains(&"Underline = Space"));
}

#[test]
fn momentary_release_emits_final_chord_and_buffer_previews_pending_output() {
    let mut guest = KeyboardComponent::new(TOUCAN).unwrap();
    let mut session = Session::new(Settings::default());
    key(&mut session, &mut guest, 19);
    let released = session.finish_momentary(Some(0), 10, &mut guest).unwrap();
    assert_eq!(
        released.strokes.as_slice(),
        &[KeyStroke::from_ascii(b'h').unwrap()]
    );
    assert!(session.model(Links::default()).detail.as_str().is_empty());
    key(&mut session, &mut guest, 19);
    let cancelled = session.finish_momentary(None, 10, &mut guest).unwrap();
    assert!(cancelled.strokes.is_empty());

    session.settings.set_binding(0, &[1, 3], b"about").unwrap();
    assert!(tap(&mut session, &mut guest, 1).strokes.is_empty());
    let buffered = session.model(Links::default());
    assert_eq!(buffered.bytes.as_slice()[10], 1);
    assert_eq!(buffered.detail.as_str(), "a");
    let expansion = tap(&mut session, &mut guest, 3);
    let expected: Vec<_> = b"about"
        .iter()
        .map(|byte| KeyStroke::from_ascii(*byte).unwrap())
        .collect();
    assert_eq!(expansion.strokes.as_slice(), expected);
}

#[test]
fn alphabet_cells_and_help_parentheses_have_separate_visual_slots() {
    let mut guest = KeyboardComponent::new(TOUCAN).unwrap();
    for (width, height, selected, m_width, prefix_width) in
        [(280, 240, 0, 30, 103), (144, 168, 12, 21, 73)]
    {
        let model = [4, selected, 0];
        let scene = guest.render(width, height, &model, "").unwrap();
        let atoms = commands(scene.as_slice());
        let find = |label: &str| {
            atoms
                .iter()
                .position(|command| command[0] == 3 && &command[8..] == label.as_bytes())
                .unwrap()
        };
        let x = |command: &[u8]| u16::from_le_bytes([command[1], command[2]]);
        let m = find("m");
        assert_eq!(atoms[m + 1][0], 4);
        assert!(x(atoms[m + 1]) >= x(atoms[m]) + m_width + 2);

        let help = find("(H)elp (");
        let dots = atoms[help + 1];
        let closing = atoms[help + 2];
        assert_eq!(dots[0], 4);
        assert_eq!(&closing[8..], b")");
        assert!(x(dots) >= x(atoms[help]) + prefix_width + 4);
        assert!(x(closing) >= x(dots) + u16::from_le_bytes([dots[5], dots[6]]) + 4);
    }
}

#[test]
fn toucan_help_teaches_the_on_keyboard_host_switch() {
    let mut guest = KeyboardComponent::new(TOUCAN).unwrap();
    let mut session = Session::new(Settings::default());
    session.enable_host_switch();
    tap(&mut session, &mut guest, LEADER);
    tap(&mut session, &mut guest, 19);
    for _ in 0..3 {
        tap(&mut session, &mut guest, 29);
    }
    assert_eq!(screen(&session), (13, 12));
    let model = session.model(Links::default());
    let scene = guest
        .render(280, 240, model.bytes.as_slice(), model.detail.as_str())
        .unwrap();
    let labels = labels(scene.as_slice());
    assert_eq!(
        labels[..5],
        [
            "Switch Host",
            "Left outer thumb",
            "first, then both",
            "Hold all three",
            "for 2 seconds"
        ],
    );
}

#[test]
fn live_preview_and_release_use_the_same_recent_held_chord() {
    use kb_core::chord::RELEASE_GRACE_MS;

    let mut guest = KeyboardComponent::new(TOUCAN).unwrap();
    for mode in [BrailleMode::Computer8, BrailleMode::Ueb1] {
        let mut session = Session::new(Settings {
            mode,
            ..Settings::default()
        });
        for (keys, time, prospective) in [
            (3, 0, "b"),
            (1, 10, "b"),
            (1, 10 + RELEASE_GRACE_MS - 1, "b"),
            (1, 10 + RELEASE_GRACE_MS, "a"),
        ] {
            assert!(
                session
                    .update(keys, time, &mut guest)
                    .unwrap()
                    .strokes
                    .is_empty()
            );
            let model = session.model(Links::default());
            assert_eq!(model.detail.as_str(), prospective);
            let scene = guest
                .render(144, 168, model.bytes.as_slice(), model.detail.as_str())
                .unwrap();
            assert_eq!(
                masks(scene.as_slice())[0],
                (if keys == 3 { 5 } else { 1 }, false)
            );
            assert_eq!(labels(scene.as_slice()), [prospective]);
        }
        let emitted = session
            .update(0, 11 + RELEASE_GRACE_MS, &mut guest)
            .unwrap();
        assert_eq!(
            emitted.strokes.as_slice(),
            &[KeyStroke::from_ascii(b'a').unwrap()]
        );
        assert!(
            session
                .update(0, 12 + RELEASE_GRACE_MS, &mut guest)
                .unwrap()
                .strokes
                .is_empty()
        );
        assert!(session.model(Links::default()).detail.as_str().is_empty());
    }
}

#[test]
fn quick_staggered_releases_stay_together_but_cancel_drops_the_remaining_chord() {
    use kb_core::chord::RELEASE_GRACE_MS;

    let mut guest = KeyboardComponent::new(TOUCAN).unwrap();
    let mut session = Session::new(Settings::default());
    for (elapsed, expected) in [(RELEASE_GRACE_MS - 1, b'b'), (RELEASE_GRACE_MS, b'a')] {
        session.update(3, 0, &mut guest).unwrap();
        session.update(1, 5, &mut guest).unwrap();
        let emitted = session.update(0, 5 + elapsed, &mut guest).unwrap();
        assert_eq!(
            emitted.strokes.as_slice(),
            &[KeyStroke::from_ascii(expected).unwrap()]
        );
    }

    session.update(3, 100, &mut guest).unwrap();
    session.update(1, 110, &mut guest).unwrap();
    session.cancel(1);
    assert!(
        session
            .update(0, 120, &mut guest)
            .unwrap()
            .strokes
            .is_empty()
    );
    assert_eq!(
        tap(&mut session, &mut guest, 1).strokes.as_slice(),
        &[KeyStroke::from_ascii(b'a').unwrap()]
    );
}

#[test]
fn correction_matches_saved_contractions_and_held_space_menu_commands() {
    use kb_core::chord::RELEASE_GRACE_MS;

    let mut guest = KeyboardComponent::new(TOUCAN).unwrap();
    let mut session = Session::new(Settings::default());
    session.settings.set_binding(0, &[1, 3], b"about").unwrap();
    assert!(tap(&mut session, &mut guest, 1).strokes.is_empty());
    session.update(7, 20, &mut guest).unwrap();
    session.update(3, 30, &mut guest).unwrap();
    let completed = session
        .update(0, 30 + RELEASE_GRACE_MS, &mut guest)
        .unwrap();
    let expansion: Vec<_> = b"about"
        .iter()
        .map(|byte| KeyStroke::from_ascii(*byte).unwrap())
        .collect();
    assert_eq!(completed.strokes.as_slice(), expansion);

    session.update(LEADER | 64, 100, &mut guest).unwrap();
    session.update(LEADER, 110, &mut guest).unwrap();
    assert!(
        session
            .update(SPACE, 110 + RELEASE_GRACE_MS, &mut guest)
            .unwrap()
            .reset
    );
    assert_eq!(screen(&session), (2, 0));
    session.update(LEADER, 170, &mut guest).unwrap();
    assert!(
        session
            .update(SPACE, 180, &mut guest)
            .unwrap()
            .strokes
            .is_empty()
    );
    assert_eq!(screen(&session), (6, 0));
    assert!(
        session
            .update(0, 185, &mut guest)
            .unwrap()
            .strokes
            .is_empty()
    );

    session.update(19 | 4, 200, &mut guest).unwrap();
    session.update(19, 210, &mut guest).unwrap();
    session
        .update(0, 210 + RELEASE_GRACE_MS, &mut guest)
        .unwrap();
    assert_eq!(screen(&session), (13, 0));
}

#[test]
fn a_released_space_expires_before_the_rest_of_the_chord() {
    use kb_core::chord::RELEASE_GRACE_MS;

    let mut guest = KeyboardComponent::new(TOUCAN).unwrap();
    let mut session = Session::new(Settings::default());
    session.update(SPACE | 1, 0, &mut guest).unwrap();
    session.update(1, 5, &mut guest).unwrap();
    let completed = session.update(0, 5 + RELEASE_GRACE_MS, &mut guest).unwrap();
    assert_eq!(
        completed.strokes.as_slice(),
        &[KeyStroke::from_ascii(b'a').unwrap()]
    );
}

#[test]
fn live_cell_and_its_reserved_preview_slot_form_a_centered_group() {
    let mut guest = KeyboardComponent::new(TOUCAN).unwrap();
    let field = |command: &[u8], offset| u16::from_le_bytes([command[offset], command[offset + 1]]);
    for (width, height, large_font, descent, body_font, former_diameter) in
        [(280, 240, 52, 9, 30, 26), (144, 168, 35, 6, 22, 17)]
    {
        let mut model = [0; 21];
        model[0] = 1;
        model[8..10].fill(255);
        let idle = guest.render(width, height, &model, "").unwrap();
        let idle = commands(idle.as_slice());
        assert_eq!(idle.len(), 1);
        assert_eq!(idle[0][9], 0);
        assert!(field(idle[0], 3) >= 25);
        assert_eq!(field(idle[0], 1) * 2 + field(idle[0], 5), width);
        assert_eq!(idle[0][10] & 0x40, 0x40);
        assert_eq!(
            (field(idle[0], 5) / 2).min(field(idle[0], 7) / 4) - 2,
            former_diameter
        );

        model[3..5].copy_from_slice(&SPACE.wrapping_add(1).to_le_bytes());
        for buffered in [false, true] {
            model[10] = if buffered { 3 } else { 0 };
            model[15..21].copy_from_slice(&[1, 0, 1, 1, 3, 0]);
            let rendered = guest.render(width, height, &model, "a").unwrap();
            let atoms = commands(rendered.as_slice());
            let cell = atoms[0];
            assert_eq!(cell[10] & 0xc0, 0xc0);
            let blank = guest.render(width, height, &model, "").unwrap();
            let blank = commands(blank.as_slice());
            assert!(blank.iter().all(|atom| atom[0] == 4));
            assert_eq!(blank[0], cell);
            if !buffered {
                assert_eq!(field(cell, 3), field(idle[0], 3));
            }
            let text = atoms.iter().find(|atom| atom[0] == 3).unwrap();
            assert_eq!(&text[8..], b"a");
            assert_eq!(text[6], 2);
            let y = field(text, 3);
            assert_eq!(y, field(cell, 3) + field(cell, 7) + 6);
            if !buffered {
                let first_ink = field(cell, 3) + 1;
                let last_ink = y + large_font - descent - 1;
                assert!((first_ink + last_ink).abs_diff(height - 1) <= 1);
            } else {
                assert!(field(atoms[1], 3) >= y + large_font + 6);
            }

            let rendered = guest
                .render(
                    width,
                    height,
                    &model,
                    "bright bold letters fill both displays",
                )
                .unwrap();
            let atoms = commands(rendered.as_slice());
            let cell = atoms[0];
            let history = &atoms[1..if buffered { 4 } else { 1 }];
            assert!(history.iter().all(|marker| marker[10] & 0x40 == 0));
            let mut end = field(cell, 3) + field(cell, 7) + 6;
            let text: Vec<_> = atoms.iter().filter(|atom| atom[0] == 3).collect();
            assert!(text.len() >= 2);
            for line in text {
                assert_eq!(line[6], 0);
                assert!(field(line, 3) >= end);
                end = field(line, 3) + body_font;
            }
            assert!(end + 8 <= height);
            assert!(history.iter().all(|marker| field(marker, 3) >= end + 6));
        }
    }
}

#[test]
fn typing_preview_leaves_no_output_blank_and_names_real_actions() {
    let mut guest = KeyboardComponent::new(TOUCAN).unwrap();
    let mut session = Session::new(Settings::default());
    key(&mut session, &mut guest, 255);
    let unknown = session.model(Links::default());
    assert!(unknown.detail.as_str().is_empty());
    for (width, height) in [(280, 240), (144, 168)] {
        let scene = guest
            .render(
                width,
                height,
                unknown.bytes.as_slice(),
                unknown.detail.as_str(),
            )
            .unwrap();
        assert!(labels(scene.as_slice()).is_empty());
        assert_eq!(masks(scene.as_slice()), [(255, false)]);
    }
    assert!(key(&mut session, &mut guest, 0).strokes.is_empty());

    session.settings.set_binding(0, &[1, 3], b"about").unwrap();
    key(&mut session, &mut guest, 1);
    assert!(session.model(Links::default()).detail.as_str().is_empty());
    key(&mut session, &mut guest, 0);
    assert_eq!(session.model(Links::default()).detail.as_str(), "a");

    session.cancel(0);
    for (cell, label) in [(SPACE, "Space"), (128, "Return"), (LEADER, "Menu prefix")] {
        key(&mut session, &mut guest, cell);
        assert_eq!(session.model(Links::default()).detail.as_str(), label);
        session.cancel(0);
    }
}
