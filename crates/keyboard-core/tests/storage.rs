use kb_core::storage::{
    COMMIT_LEN, HEADER_LEN, MAGIC, MAX_ENCODED_LEN, MAX_PAYLOAD_LEN, StorageError, decode, encode,
    generation_is_newer, select_newest,
};

#[test]
fn every_payload_size_round_trips_in_aligned_storage() {
    let payload = [0x5a; MAX_PAYLOAD_LEN];
    for size in 0..=MAX_PAYLOAD_LEN {
        let mut bytes = [0; MAX_ENCODED_LEN];
        let len = encode(&payload[..size], 123, &mut bytes).unwrap();
        assert_eq!(len % 4, 0);
        assert_eq!(
            &bytes[HEADER_LEN + size..len],
            &[0xff; 3][..len - HEADER_LEN - size]
        );
        let envelope = decode(&bytes[..len]).unwrap();
        assert_eq!(envelope.generation, 123);
        assert_eq!(envelope.payload, &payload[..size]);
    }
}

#[test]
fn interrupted_writes_never_replace_the_previous_valid_slot() {
    let mut old_slot = [0xff; MAX_ENCODED_LEN];
    encode(b"previous bond", 7, &mut old_slot).unwrap();
    let mut encoded = [0xff; MAX_ENCODED_LEN];
    let len = encode(b"new bond", 8, &mut encoded).unwrap();
    for written in (COMMIT_LEN..=len).step_by(4) {
        let mut new_slot = [0xff; MAX_ENCODED_LEN];
        new_slot[COMMIT_LEN..written].copy_from_slice(&encoded[COMMIT_LEN..written]);
        let selected = select_newest(&old_slot, &new_slot).unwrap();
        assert_eq!(selected.slot, 0);
        assert_eq!(selected.envelope.payload, b"previous bond");
        for commit_bytes in 0..COMMIT_LEN {
            new_slot[..commit_bytes].copy_from_slice(&encoded[..commit_bytes]);
            assert_eq!(select_newest(&old_slot, &new_slot).unwrap().slot, 0);
        }
    }
    let mut new_slot = [0xff; MAX_ENCODED_LEN];
    new_slot[COMMIT_LEN..len].copy_from_slice(&encoded[COMMIT_LEN..len]);
    new_slot[..COMMIT_LEN].copy_from_slice(&MAGIC);
    assert_eq!(
        select_newest(&old_slot, &new_slot)
            .unwrap()
            .envelope
            .payload,
        b"new bond"
    );
}

#[test]
fn all_header_and_payload_corruption_is_detected() {
    let mut encoded = [0xff; MAX_ENCODED_LEN];
    encode(b"bond with keys", 3, &mut encoded).unwrap();
    for index in 0..HEADER_LEN + b"bond with keys".len() {
        for bit in 0..8 {
            let mut corrupted = encoded;
            corrupted[index] ^= 1 << bit;
            assert!(decode(&corrupted).is_err(), "index {index}, bit {bit}");
        }
    }
}

#[test]
fn newest_selection_survives_generation_wrap_and_corrupt_slots() {
    let mut first = [0xff; MAX_ENCODED_LEN];
    let mut second = [0xff; MAX_ENCODED_LEN];
    assert_eq!(select_newest(&first, &second), None);
    encode(b"before wrap", u32::MAX, &mut first).unwrap();
    assert_eq!(select_newest(&first, &second).unwrap().slot, 0);
    encode(b"after wrap", 0, &mut second).unwrap();
    assert_eq!(select_newest(&first, &second).unwrap().slot, 1);
    second[HEADER_LEN] ^= 1;
    assert_eq!(select_newest(&first, &second).unwrap().slot, 0);
    first[0] = 0;
    assert_eq!(select_newest(&first, &second), None);
}

#[test]
fn equal_or_ambiguous_generations_have_deterministic_selection() {
    assert!(!generation_is_newer(1, 1));
    assert!(!generation_is_newer(0x8000_0000, 0));
    assert!(generation_is_newer(0, u32::MAX));
    assert!(!generation_is_newer(u32::MAX, 0));
    let mut first = [0xff; MAX_ENCODED_LEN];
    let mut second = [0xff; MAX_ENCODED_LEN];
    encode(b"first", 42, &mut first).unwrap();
    encode(b"second", 42, &mut second).unwrap();
    assert_eq!(select_newest(&first, &second).unwrap().slot, 0);
}

#[test]
fn oversized_or_short_buffers_fail_before_mutation() {
    let mut destination = [0x19; MAX_ENCODED_LEN];
    assert_eq!(
        encode(&[0; MAX_PAYLOAD_LEN + 1], 0, &mut destination),
        Err(StorageError::PayloadTooLarge)
    );
    assert_eq!(destination, [0x19; MAX_ENCODED_LEN]);
    assert_eq!(
        encode(b"test", 0, &mut destination[..HEADER_LEN]),
        Err(StorageError::BufferTooSmall)
    );
    assert_eq!(destination, [0x19; MAX_ENCODED_LEN]);
    for size in 0..HEADER_LEN {
        assert_eq!(
            decode(&destination[..size]),
            Err(StorageError::BufferTooSmall)
        );
    }
}

#[test]
fn completing_an_older_save_does_not_mark_newer_settings_persisted() {
    let (state, first) = kb_core::storage::SaveState::new().requested();
    let (state, second) = state.requested();
    let state = state.completed(first, true);
    assert!(state.unsaved());
    assert!(!state.completed(second, true).unsaved());
}

#[test]
fn a_later_successful_save_recovers_from_failed_flash_storage() {
    let (state, first) = kb_core::storage::SaveState::new().requested();
    let state = state.completed(first, false);
    assert!(state.unsaved());
    let (state, replacement) = state.requested();
    assert!(!state.completed(replacement, true).unsaved());
}
