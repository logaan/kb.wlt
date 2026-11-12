use core::convert::Infallible;
use kb_core::{computer8, reference::*};

#[derive(Default)]
struct Host {
    controller: State,
    chord: u16,
    output: Vec<u32>,
    time: u32,
}

impl Host {
    fn keys(&mut self, keys: u16) -> Action {
        self.time = self.time.wrapping_add(1);
        let transition = self.controller.step(keys, self.time);
        self.controller = transition.state;
        if transition.reset_braille {
            self.chord = 0;
        }
        if transition.forward {
            self.chord |= keys;
            if keys == 0 {
                if let Some(stroke) = computer8(self.chord) {
                    self.output.push(stroke.packed());
                }
                self.chord = 0;
            }
        }
        transition.action
    }

    fn chord(&mut self, cell: u16) -> Action {
        self.keys(cell);
        self.keys(0)
    }

    fn enter(&mut self) {
        assert_eq!(self.chord(LEADER), Action::Namespace);
        assert_eq!(self.chord(REFERENCE), Action::Enter);
    }
}

fn catalog() -> Catalog {
    Catalog::try_from_fn(|cell| Ok::<_, Infallible>(computer8(cell).map_or(0, |key| key.packed())))
        .unwrap()
}

#[test]
fn held_space_segments_q_and_r_without_accumulating_them() {
    let mut host = Host::default();
    host.keys(SPACE);
    host.keys(LEADER);
    assert_eq!(host.keys(SPACE), Action::Namespace);
    host.keys(REFERENCE);
    assert_eq!(host.keys(SPACE), Action::Enter);
    host.keys(0);
    assert_eq!(host.controller.mode, Mode::Reference);
    assert!(host.output.is_empty());
    assert_eq!(host.chord(1), Action::Lookup(1));
    assert!(host.output.is_empty());
}

#[test]
fn released_space_namespace_and_exit_never_reach_host() {
    let mut host = Host::default();
    host.chord(1);
    let before = host.output.clone();
    host.enter();
    assert_eq!(host.chord(0x101), Action::Previous);
    assert_eq!(host.chord(0x108), Action::Next);
    assert_eq!(host.chord(0x103), Action::Exit);
    assert_eq!(host.output, before);
    host.keys(0);
    host.chord(3);
    assert_eq!(host.output.len(), before.len() + 1);
    assert_eq!(host.output.last(), Some(&computer8(3).unwrap().packed()));
}

#[test]
fn every_release_order_captures_the_leader_before_output() {
    fn check(host: Host, remaining: u16) {
        if remaining == 0 {
            assert_eq!(host.controller.mode, Mode::Namespace);
            assert!(host.output.is_empty());
            return;
        }
        for bit in 0..9 {
            let bit = 1 << bit;
            if remaining & bit == 0 {
                continue;
            }
            let mut next = Host {
                controller: host.controller,
                chord: host.chord,
                output: host.output.clone(),
                time: host.time,
            };
            next.keys(remaining & !bit);
            check(next, remaining & !bit);
        }
    }
    let mut host = Host::default();
    host.keys(LEADER);
    check(host, LEADER);
}

#[test]
fn unknown_and_reserved_namespaces_are_swallowed_until_neutral() {
    for command in [0x109, 0x101, 1, 0x117 & !SPACE] {
        let mut host = Host::default();
        assert_eq!(host.chord(LEADER), Action::Namespace);
        assert_eq!(host.chord(command), Action::Rejected);
        host.keys(0);
        host.chord(3);
        assert_eq!(host.output, [computer8(3).unwrap().packed()]);
    }
}

#[test]
fn timeouts_and_disconnects_discard_partial_chords_and_held_space() {
    for mode in [Mode::Namespace, Mode::Reference] {
        let mut host = Host::default();
        host.chord(LEADER);
        if mode == Mode::Reference {
            host.chord(REFERENCE);
        }
        host.keys(SPACE);
        host.keys(0x103);
        let transition = if mode == Mode::Namespace {
            host.controller
                .step(0x103, host.time + NAMESPACE_TIMEOUT_MS)
        } else {
            host.controller.cancel(0x103)
        };
        assert!(!transition.forward);
        host.controller = transition.state;
        host.chord = 0;
        host.keys(SPACE);
        host.keys(0x101);
        host.keys(SPACE);
        host.keys(0);
        host.chord(3);
        assert_eq!(host.output, [computer8(3).unwrap().packed()]);
    }
}

#[test]
fn namespace_timeout_wraps_safely_and_does_not_release_space() {
    let mut state = State::new().step(LEADER, u32::MAX - 5).state;
    state = state.step(SPACE, u32::MAX - 4).state;
    assert_eq!(state.mode, Mode::Namespace);
    assert_eq!(state.step(SPACE, 10).state.mode, Mode::Namespace);
    let next = state.step(SPACE, 2_000);
    assert_eq!(next.state.mode, Mode::ReleaseGuard);
    assert!(!next.forward);
    assert!(next.reset_braille);
}

#[test]
fn exit_with_space_held_does_not_emit_space_or_next_partial_chord() {
    let mut host = Host::default();
    host.enter();
    host.keys(SPACE);
    host.keys(0x103);
    assert_eq!(host.keys(SPACE), Action::Exit);
    host.keys(0x101);
    host.keys(SPACE);
    host.keys(0);
    host.chord(1);
    assert_eq!(host.output, [computer8(1).unwrap().packed()]);
}

#[test]
fn find_accepts_navigation_chords_as_literal_mapping_queries() {
    let mut host = Host::default();
    host.enter();
    assert_eq!(host.chord(0x10b), Action::Find);
    assert_eq!(host.controller.mode, Mode::Find);
    assert_eq!(host.chord(0x103), Action::Lookup(0x103));
    assert_eq!(host.controller.mode, Mode::Reference);
    assert!(host.output.is_empty());
}

#[test]
fn plain_typing_and_space_commands_keep_existing_semantics() {
    let mut host = Host::default();
    for cell in [1, 3, 0x1f, SPACE, 0x140, 0x40, 0x101] {
        host.chord(cell);
    }
    assert_eq!(host.output.len(), 7);
    assert_eq!(host.output[2], computer8(0x1f).unwrap().packed());
    assert_eq!(host.output[3], computer8(SPACE).unwrap().packed());
}

#[test]
fn catalog_enumerates_all_nonzero_custom_mappings_without_filtering() {
    let mut seen = Vec::new();
    let catalog = Catalog::try_from_fn(|cell| {
        seen.push(cell);
        Ok::<_, Infallible>(if cell % 2 == 0 {
            u32::from(cell) << 8
        } else {
            0
        })
    })
    .unwrap();
    assert_eq!(seen, (1..=511).collect::<Vec<_>>());
    assert_eq!(catalog.len(), 255);
    assert_eq!(
        catalog.entries()[254],
        Entry {
            cell: 510,
            packed: 510 << 8
        }
    );
    assert_eq!(catalog.find(510), Some(254));
    assert_eq!(catalog.find(511), None);
}

#[test]
fn browser_reaches_every_mapping_and_lookup_reports_missing_cells() {
    let catalog = catalog();
    let mut cursor = Cursor::default();
    for index in 0..catalog.len() {
        assert_eq!(cursor.selected, index);
        let page = cursor.page(&catalog, Mode::Reference);
        assert_eq!(
            page.entries[usize::from(page.selected)],
            catalog.entries()[usize::from(index)]
        );
        cursor = cursor.apply(Action::Next, &catalog);
    }
    assert_eq!(cursor.selected, catalog.len() - 1);
    cursor = cursor.apply(Action::Lookup(LEADER), &catalog);
    let page = cursor.page(&catalog, Mode::Reference);
    assert_eq!(page.lookup, Some(LEADER));
    assert!(!page.lookup_found);
    cursor = cursor.apply(Action::Lookup(SPACE), &catalog);
    assert!(cursor.page(&catalog, Mode::Reference).lookup_found);
    let empty = Catalog::try_from_fn(|_| Ok::<_, Infallible>(0)).unwrap();
    assert!(empty.is_empty());
    assert_eq!(
        Cursor::default()
            .apply(Action::Last, &empty)
            .page(&empty, Mode::Reference)
            .len,
        0
    );
}

#[test]
fn six_and_eight_dot_cells_keep_space_separate() {
    for cell in [1, 0x3f, SPACE, 0x13f] {
        assert_eq!(Entry { cell, packed: 1 }.dot_rows(), 3);
    }
    for cell in [0x40, 0x80, 0xc0, 0x140] {
        assert_eq!(Entry { cell, packed: 1 }.dot_rows(), 4);
    }
    assert!(
        !Entry {
            cell: 0x80,
            packed: 1
        }
        .has_space()
    );
    assert!(
        Entry {
            cell: SPACE,
            packed: 1
        }
        .has_space()
    );
}
