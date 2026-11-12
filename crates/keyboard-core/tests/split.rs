use kb_core::split::{Layout, PACKET_LEN, PacketError, ReceiveError, Receiver, Snapshot};

fn snapshot(node: u64, layout: Layout, sequence: u16, keys: u8) -> Snapshot {
    Snapshot {
        node,
        boot: 77,
        sequence,
        layout,
        keys,
    }
}

#[test]
fn snapshots_fit_a_default_att_payload_and_detect_corruption() {
    let original = snapshot(0xfedc_ba98_7654_3210, Layout::Right, u16::MAX, 0xa5);
    let encoded = original.encode();
    assert_eq!(encoded.len(), PACKET_LEN);
    assert_eq!(Snapshot::decode(&encoded), Ok(original));
    for index in 0..PACKET_LEN {
        for bit in 0..8 {
            let mut corrupt = encoded;
            corrupt[index] ^= 1 << bit;
            assert!(Snapshot::decode(&corrupt).is_err());
        }
    }
    assert_eq!(Snapshot::decode(&encoded[..19]), Err(PacketError::Length));
}

#[test]
fn layout_maps_single_and_split_top_rows_to_the_same_braille_cells() {
    assert_eq!(Layout::Single.map(0xff), 0x17f);
    assert_eq!(Layout::Left.map(0xff), 0x47);
    assert_eq!(Layout::Right.map(0xff), 0x138);
    assert_eq!(
        Layout::Left.map(0x0f) | Layout::Right.map(0x0f),
        Layout::Single.map(0xff)
    );
    assert_eq!(Layout::Single.map(0x80), 0x100);
    assert_eq!(Layout::Right.map(0x08), 0x100);
}

#[test]
fn reconnecting_while_pressed_requires_neutral_before_accepting_keys() {
    let (receiver, session, _) = Receiver::<1>::new(500)
        .connect(0, 10, Layout::Single, 0)
        .unwrap();
    let (receiver, update) = receiver
        .receive(session, snapshot(10, Layout::Single, 1, 1), 1)
        .unwrap();
    assert_eq!(update.keys, 0);
    let (receiver, update) = receiver
        .receive(session, snapshot(10, Layout::Single, 2, 3), 2)
        .unwrap();
    assert_eq!(update.keys, 0);
    let (receiver, update) = receiver
        .receive(session, snapshot(10, Layout::Single, 3, 0), 3)
        .unwrap();
    assert_eq!(update.keys, 0);
    let (_, update) = receiver
        .receive(session, snapshot(10, Layout::Single, 4, 1), 4)
        .unwrap();
    assert_eq!(update.pressed, 0x40);
}

#[test]
fn independent_halves_union_keys_and_full_snapshots_repair_lost_edges() {
    let (receiver, left, _) = Receiver::<2>::new(500)
        .connect(0, 10, Layout::Left, 0)
        .unwrap();
    let (receiver, right, _) = receiver.connect(1, 20, Layout::Right, 0).unwrap();
    let (receiver, _) = receiver
        .receive(left, snapshot(10, Layout::Left, 1, 0), 0)
        .unwrap();
    let (receiver, _) = receiver
        .receive(right, snapshot(20, Layout::Right, 1, 0), 0)
        .unwrap();
    let (receiver, update) = receiver
        .receive(left, snapshot(10, Layout::Left, 2, 8), 1)
        .unwrap();
    assert_eq!(update.keys, 1);
    let (receiver, update) = receiver
        .receive(right, snapshot(20, Layout::Right, 2, 1), 2)
        .unwrap();
    assert_eq!(update.keys, 9);
    let (receiver, update) = receiver
        .receive(left, snapshot(10, Layout::Left, 6, 0), 3)
        .unwrap();
    assert_eq!((update.keys, update.released), (8, 1));
    let (_, update) = receiver
        .receive(right, snapshot(20, Layout::Right, 8, 0), 4)
        .unwrap();
    assert_eq!((update.keys, update.released), (0, 8));
}

#[test]
fn duplicate_bindings_do_not_release_until_both_physical_sources_release() {
    let (receiver, first, _) = Receiver::<2>::new(500)
        .connect(0, 10, Layout::Single, 0)
        .unwrap();
    let (receiver, second, _) = receiver.connect(1, 20, Layout::Single, 0).unwrap();
    let (receiver, _) = receiver
        .receive(first, snapshot(10, Layout::Single, 1, 0), 0)
        .unwrap();
    let (receiver, _) = receiver
        .receive(second, snapshot(20, Layout::Single, 1, 0), 0)
        .unwrap();
    let (receiver, _) = receiver
        .receive(first, snapshot(10, Layout::Single, 2, 8), 1)
        .unwrap();
    let (receiver, _) = receiver
        .receive(second, snapshot(20, Layout::Single, 2, 8), 1)
        .unwrap();
    let (receiver, update) = receiver
        .receive(first, snapshot(10, Layout::Single, 3, 0), 2)
        .unwrap();
    assert_eq!(update.released, 0);
    let (_, update) = receiver
        .receive(second, snapshot(20, Layout::Single, 3, 0), 3)
        .unwrap();
    assert_eq!(update.released, 1);
}

#[test]
fn sequence_wrap_is_accepted_but_duplicates_and_old_snapshots_are_rejected() {
    let (receiver, session, _) = Receiver::<1>::new(500)
        .connect(0, 10, Layout::Single, 0)
        .unwrap();
    let (receiver, _) = receiver
        .receive(session, snapshot(10, Layout::Single, u16::MAX, 0), 0)
        .unwrap();
    let (receiver, _) = receiver
        .receive(session, snapshot(10, Layout::Single, 0, 1), 1)
        .unwrap();
    for sequence in [0, u16::MAX, 0x8000] {
        assert_eq!(
            receiver.receive(session, snapshot(10, Layout::Single, sequence, 0), 2),
            Err(ReceiveError::Sequence)
        );
    }
}

#[test]
fn stale_connection_tokens_and_changed_device_identity_are_rejected() {
    let (receiver, old, _) = Receiver::<1>::new(500)
        .connect(0, 10, Layout::Single, 0)
        .unwrap();
    let (receiver, session, _) = receiver.connect(0, 10, Layout::Single, 1).unwrap();
    assert_eq!(
        receiver.receive(old, snapshot(10, Layout::Single, 0, 0), 2),
        Err(ReceiveError::StaleSession)
    );
    assert_eq!(
        receiver.receive(session, snapshot(11, Layout::Single, 0, 0), 2),
        Err(ReceiveError::Identity)
    );
    assert_eq!(
        receiver.receive(session, snapshot(10, Layout::Left, 0, 0), 2),
        Err(ReceiveError::Layout)
    );
    let (receiver, _) = receiver
        .receive(session, snapshot(10, Layout::Single, 0, 0), 2)
        .unwrap();
    let mut reboot = snapshot(10, Layout::Single, 1, 0);
    reboot.boot += 1;
    assert_eq!(
        receiver.receive(session, reboot, 3),
        Err(ReceiveError::BootChanged)
    );
}

#[test]
fn silence_and_disconnect_cancel_partial_chords_and_release_keys() {
    let start = u32::MAX - 10;
    let (receiver, session, _) = Receiver::<1>::new(20)
        .connect(0, 10, Layout::Single, start)
        .unwrap();
    let (receiver, _) = receiver
        .receive(session, snapshot(10, Layout::Single, 0, 0), start)
        .unwrap();
    let (receiver, _) = receiver
        .receive(session, snapshot(10, Layout::Single, 1, 8), start + 1)
        .unwrap();
    let (live, update) = receiver.expire(9);
    assert_eq!(update.released, 0);
    assert!(live.is_connected(0));
    let (expired, update) = receiver.expire(10);
    assert_eq!(update.released, 1);
    assert!(update.cancel_chord);
    assert!(!expired.is_connected(0));
    assert_eq!(
        expired.receive(session, snapshot(10, Layout::Single, 2, 0), 11),
        Err(ReceiveError::StaleSession)
    );
    let (_, update) = live.disconnect(session).unwrap();
    assert_eq!(update.released, 1);
    assert!(update.cancel_chord);
}

#[test]
fn replacing_closing_and_expiring_idle_links_cancel_pending_input() {
    let receiver = Receiver::<1>::new(500);
    let (receiver, session, update) = receiver.connect(0, 1, Layout::Single, 0).unwrap();
    assert!(!update.cancel_chord);
    assert!(receiver.disconnect(session).unwrap().1.cancel_chord);
    assert!(receiver.expire(500).1.cancel_chord);
    assert!(
        receiver
            .connect(0, 2, Layout::Single, 499)
            .unwrap()
            .2
            .cancel_chord
    );
}
