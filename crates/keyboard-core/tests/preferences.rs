use kb_core::{
    KeyStroke,
    preferences::{BrailleMode, ENCODED_LEN, Engine, Error, PREFIX_TIMEOUT_MS, Settings},
};

fn ascii(text: &str) -> Vec<KeyStroke> {
    text.bytes()
        .map(|byte| KeyStroke::from_ascii(byte).unwrap())
        .collect()
}

fn normal(cell: u16) -> Option<KeyStroke> {
    kb_core::computer8(cell)
}

#[test]
fn all_binding_slots_round_trip_inside_the_flash_payload() {
    let mut settings = Settings {
        mode: BrailleMode::Ueb1,
        ..Settings::default()
    };
    for slot in 0..12 {
        settings
            .set_binding(
                slot,
                &[slot as u16 + 1, 2, 3, 4],
                b"abcdefghijklmnopqrstuvwx",
            )
            .unwrap();
    }
    let mut bytes = [0; 512];
    let len = settings.encode(&mut bytes).unwrap();
    assert_eq!(len, ENCODED_LEN);
    assert!(len <= 512);
    assert_eq!(Settings::decode(&bytes[..len]), Ok(settings));
    for truncated in 0..len {
        assert!(Settings::decode(&bytes[..truncated]).is_err());
    }
    assert!(Settings::decode(&bytes[..len + 1]).is_err());
}

#[test]
fn invalid_edits_are_transactional_and_namespace_is_reserved() {
    let mut settings = Settings::default();
    settings.set_binding(0, &[1, 2], b"hello").unwrap();
    let before = settings;
    for (trigger, text) in [
        (&[][..], &b"text"[..]),
        (&[0][..], &b"text"[..]),
        (&[512][..], &b"text"[..]),
        (&[1][..], &b""[..]),
        (&[1][..], &b"\xff"[..]),
    ] {
        assert!(settings.set_binding(0, trigger, text).is_err());
        assert_eq!(settings, before);
    }
    assert_eq!(
        settings.set_binding(1, &[1, 2], b"duplicate"),
        Err(Error::DuplicateTrigger)
    );
    assert_eq!(
        settings.set_binding(1, &[0x11f], b"reserved"),
        Err(Error::ReservedChord)
    );
    settings.clear_binding(0).unwrap();
    assert!(settings.bindings[0].is_empty());
}

#[test]
fn unknown_or_noncanonical_encodings_do_not_load() {
    let mut bytes = [0; ENCODED_LEN];
    Settings::default().encode(&mut bytes).unwrap();
    for (offset, value) in [(0, 0), (4, 2), (5, 2), (6, 5), (7, 25), (8, 1), (16, 1)] {
        let mut corrupt = bytes;
        corrupt[offset] = value;
        assert!(Settings::decode(&corrupt).is_err(), "offset {offset}");
    }
}

#[test]
fn single_chords_and_longest_contractions_expand() {
    let mut settings = Settings::default();
    settings.set_binding(0, &[1], b"a short").unwrap();
    settings
        .set_binding(1, &[1, 3, 9], b"a longer expansion")
        .unwrap();
    let mut engine = Engine::default();
    assert!(engine.accept(&settings, 1, normal(1), 0).is_empty());
    assert!(engine.accept(&settings, 3, normal(3), 100).is_empty());
    assert_eq!(
        engine.accept(&settings, 9, normal(9), 200).as_slice(),
        ascii("a longer expansion")
    );
    assert_eq!(engine.pending_len(), 0);
}

#[test]
fn mismatches_replay_the_actual_component_fallback_in_order() {
    let mut settings = Settings::default();
    settings.set_binding(0, &[1, 3], b"hello").unwrap();
    let mut engine = Engine::default();
    let custom = Some(KeyStroke::new(108, 0x40));
    assert!(engine.accept(&settings, 1, custom, 0).is_empty());
    assert_eq!(
        engine.accept(&settings, 9, normal(9), 1).as_slice(),
        [custom.unwrap(), normal(9).unwrap()]
    );
    assert_eq!(
        engine.accept(&settings, 256, normal(256), 2).as_slice(),
        ascii(" ")
    );
}

#[test]
fn tentative_component_batches_are_replayed_or_replaced_atomically() {
    use kb_core::preferences::Fallback;
    let mut settings = Settings::default();
    settings
        .set_binding(0, &[1, 2, 3, 5], b"replacement")
        .unwrap();
    let mut engine = Engine::default();
    let batch = |cell: u16| {
        Fallback::from_slice(
            &[KeyStroke::new(cell as u8 + 4, 0); 24],
            KeyStroke::default(),
        )
        .unwrap()
    };
    for cell in 1..=3 {
        assert!(
            engine
                .accept_batch(&settings, cell, batch(cell), cell as u32)
                .is_empty()
        );
    }
    let replay = engine.accept_batch(&settings, 4, batch(4), 4);
    let expected = (1..=4)
        .flat_map(|cell| batch(cell).as_slice().to_vec())
        .collect::<Vec<_>>();
    assert_eq!(replay.len(), 96);
    assert_eq!(replay.as_slice(), expected);
    for cell in 1..=3 {
        assert!(
            engine
                .accept_batch(&settings, cell, batch(cell), cell as u32 + 10)
                .is_empty()
        );
    }
    assert_eq!(
        engine.accept_batch(&settings, 5, batch(5), 15).as_slice(),
        ascii("replacement")
    );
}

#[test]
fn timeout_chooses_complete_prefix_and_handles_clock_wrap() {
    let mut settings = Settings::default();
    settings.set_binding(0, &[1], b"short").unwrap();
    settings.set_binding(1, &[1, 3], b"long").unwrap();
    let mut engine = Engine::default();
    let start = u32::MAX - 100;
    assert!(engine.accept(&settings, 1, normal(1), start).is_empty());
    assert!(
        engine
            .poll(&settings, start.wrapping_add(PREFIX_TIMEOUT_MS - 1))
            .is_empty()
    );
    assert_eq!(
        engine
            .poll(&settings, start.wrapping_add(PREFIX_TIMEOUT_MS))
            .as_slice(),
        ascii("short")
    );
    assert!(
        engine
            .poll(&settings, start.wrapping_add(PREFIX_TIMEOUT_MS + 1))
            .is_empty()
    );
}

#[test]
fn simultaneous_expiry_and_new_expansion_do_not_truncate() {
    let mut settings = Settings::default();
    settings.set_binding(0, &[1], &[b'a'; 24]).unwrap();
    settings.set_binding(1, &[1, 2], b"long").unwrap();
    settings.set_binding(2, &[3], &[b'c'; 24]).unwrap();
    let mut engine = Engine::default();
    assert!(engine.accept(&settings, 1, normal(1), 0).is_empty());
    let output = engine.accept(&settings, 3, normal(3), PREFIX_TIMEOUT_MS);
    assert_eq!(output.len(), 48);
    assert_eq!(&output.as_slice()[..24], ascii(&"a".repeat(24)));
    assert_eq!(&output.as_slice()[24..], ascii(&"c".repeat(24)));
}

#[test]
fn a_failed_long_prefix_preserves_four_maximum_expansions() {
    let mut settings = Settings::default();
    settings
        .set_binding(0, &[1, 2, 3, 5], b"long match")
        .unwrap();
    for slot in 1..=4 {
        settings
            .set_binding(slot, &[slot as u16], &[b'a' + slot as u8 - 1; 24])
            .unwrap();
    }
    let mut engine = Engine::default();
    for cell in 1..=3 {
        assert!(
            engine
                .accept(&settings, cell, normal(cell), cell as u32)
                .is_empty()
        );
    }
    let output = engine.accept(&settings, 4, normal(4), 4);
    assert_eq!(output.len(), 96);
    let expected = (b'a'..=b'd')
        .flat_map(|byte| [KeyStroke::from_ascii(byte).unwrap(); 24])
        .collect::<Vec<_>>();
    assert_eq!(output.as_slice(), expected);

    for cell in 1..=3 {
        assert!(
            engine
                .accept(&settings, cell, normal(cell), 10 + cell as u32)
                .is_empty()
        );
    }
    let expired = engine.accept(&settings, 4, normal(4), 13 + PREFIX_TIMEOUT_MS);
    assert_eq!(expired.as_slice(), expected);
}

#[test]
fn cancellation_and_mode_changes_discard_prefixes() {
    let mut settings = Settings::default();
    settings.set_binding(0, &[1, 3], b"expansion").unwrap();
    let mut engine = Engine::default();
    engine.accept(&settings, 1, normal(1), 0);
    engine.reset();
    assert!(engine.poll(&settings, PREFIX_TIMEOUT_MS).is_empty());
    engine.accept(&settings, 1, normal(1), 0);
    settings.mode = BrailleMode::Ueb1;
    assert!(engine.poll(&settings, PREFIX_TIMEOUT_MS).is_empty());
    assert!(engine.accept(&settings, 32, normal(32), 2_000).is_empty());
    assert_eq!(
        engine.accept(&settings, 9, normal(9), 2_001).as_slice(),
        ascii("C")
    );
}

#[test]
fn ueb_context_is_evaluated_after_binding_prefixes_resolve() {
    let mut settings = Settings {
        mode: BrailleMode::Ueb1,
        ..Settings::default()
    };
    settings.set_binding(0, &[1, 3], b"hello ").unwrap();
    let mut engine = Engine::default();
    assert!(engine.accept(&settings, 60, normal(60), 0).is_empty());
    assert!(engine.accept(&settings, 1, normal(1), 1).is_empty());
    assert_eq!(
        engine.accept(&settings, 3, normal(3), 2).as_slice(),
        ascii("hello ")
    );
    assert_eq!(
        engine.accept(&settings, 9, normal(9), 3).as_slice(),
        ascii("c")
    );
}
