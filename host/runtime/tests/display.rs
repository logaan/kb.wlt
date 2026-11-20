use kb_client_session::{Change, Guest, Links, Model, Session};
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
            1 | 2 | 4 => 11,
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
        .map(|command| (command[9], command[10] != 0))
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
fn both_production_clients_render_graphical_home_status() {
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
            let scene = commands(scene.as_slice());
            assert!(scene.iter().all(|command| matches!(command[0], 1 | 2)));
            assert!(scene.iter().any(|command| command[9] == 5));
            assert!(scene.iter().any(|command| command[9] == 6));
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
    assert_eq!(masks(held.as_slice()), [(19, false)]);
    assert_eq!(labels(held.as_slice()), ["h"]);
    assert_eq!(
        key(&mut session, &mut guest, 0).strokes.as_slice(),
        &[KeyStroke::from_ascii(b'h').unwrap()],
    );
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
fn reference_keeps_four_by_four_letters_and_consistent_nav() {
    let mut guest = BrailleComponent::new(PAINTBRUSH).unwrap();
    for (width, height) in [(144, 168), (280, 240)] {
        let page = guest.render(width, height, &[4, 0, 1], "").unwrap();
        let labels = labels(page.as_slice());
        assert_eq!(labels[0], "Capitals");
        assert_eq!(
            labels[1..17],
            [
                "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P",
            ],
        );
        assert_eq!(labels[17], "(H)elp (");
        assert_eq!(masks(page.as_slice()).last(), Some(&(19, false)));

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
            transition(guest.navigate(4, 256, cell, 16).unwrap()),
            expected,
        );
    }
}
