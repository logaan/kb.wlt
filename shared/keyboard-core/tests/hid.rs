use kb_core::hid::RELEASE_REPORT;
use kb_core::{KeyStroke, QueueFull, ReportQueue};

#[test]
fn shifted_taps_always_include_a_complete_release() {
    let mut queue = ReportQueue::<4>::new();
    queue
        .enqueue_tap(KeyStroke::from_ascii(b'A').unwrap())
        .unwrap();
    queue
        .enqueue_tap(KeyStroke::from_ascii(b'a').unwrap())
        .unwrap();
    assert_eq!(queue.pop(), Some([2, 0, 4, 0, 0, 0, 0, 0]));
    assert_eq!(queue.pop(), Some(RELEASE_REPORT));
    assert_eq!(queue.pop(), Some([0, 0, 4, 0, 0, 0, 0, 0]));
    assert_eq!(queue.pop(), Some(RELEASE_REPORT));
    assert_eq!(queue.pop(), None);
}

#[test]
fn full_queue_rejects_both_parts_of_a_tap_atomically() {
    let mut queue = ReportQueue::<3>::new();
    let stroke = KeyStroke::new(4, 0);
    queue.enqueue_tap(stroke).unwrap();
    let before = queue;
    assert_eq!(queue.enqueue_tap(stroke), Err(QueueFull));
    assert_eq!(queue, before);
    assert_eq!(queue.remaining_capacity(), 1);
}

#[test]
fn failed_transfers_can_retry_the_front_without_reordering() {
    let mut queue = ReportQueue::<2>::new();
    let stroke = KeyStroke::new(5, 0);
    queue.enqueue_tap(stroke).unwrap();
    for _ in 0..5 {
        assert_eq!(queue.front(), Some(&stroke.report()));
    }
    queue.pop();
    assert_eq!(queue.front(), Some(&RELEASE_REPORT));
    queue.pop();
    queue.enqueue_tap(stroke).unwrap();
    assert_eq!(queue.pop(), Some(stroke.report()));
}

#[test]
fn empty_capacity_rejects_without_panicking() {
    let mut queue = ReportQueue::<0>::new();
    assert_eq!(queue.enqueue(RELEASE_REPORT), Err(QueueFull));
    assert_eq!(queue.enqueue_tap(KeyStroke::new(4, 0)), Err(QueueFull));
    assert_eq!(queue.pop(), None);
}

#[test]
fn packing_preserves_modifiers_and_rejects_invalid_values() {
    for modifier in 0..=255 {
        let stroke = KeyStroke::new(4, modifier);
        assert_eq!(KeyStroke::from_packed(stroke.packed()), Some(stroke));
    }
    for packed in [
        0,
        1,
        2,
        3,
        0x102,
        0x40000004,
        0x80000000,
        0x80000004,
        u32::MAX,
    ] {
        assert_eq!(KeyStroke::from_packed(packed), None);
    }
}

#[test]
fn packed_commands_distinguish_sticky_from_tap_and_keep_held_internal() {
    for stroke in [
        KeyStroke::new(0, 2).sticky(),
        KeyStroke::new(0, 0).with_consumer(0x029d).sticky(),
        KeyStroke::new(0, 2),
        KeyStroke::new(8, 0).with_consumer(0x029d),
        KeyStroke::new(0, 0).with_consumer(0x029d),
    ] {
        assert_eq!(KeyStroke::from_packed(stroke.packed()), Some(stroke));
        assert_eq!(stroke.consumer_report(), stroke.consumer.to_le_bytes());
        assert!(!stroke.is_held());
    }
    assert_eq!(
        KeyStroke::from_packed(KeyStroke::new(0, 2).held().packed()),
        None
    );
}

fn descriptor_reports(bytes: &[u8]) -> std::collections::BTreeMap<(u8, u8), u32> {
    let mut reports = std::collections::BTreeMap::new();
    let (mut index, mut id, mut size, mut count) = (0, 0, 0, 0);
    while index < bytes.len() {
        let prefix = bytes[index];
        let len = match prefix & 3 {
            3 => 4,
            value => value as usize,
        };
        let mut value = 0;
        for offset in 0..len {
            value |= u32::from(bytes[index + offset + 1]) << (offset * 8);
        }
        match prefix & 0xfc {
            0x74 => size = value,
            0x94 => count = value,
            0x84 => id = value as u8,
            0x80 | 0x90 => *reports.entry((id, prefix & 0xfc)).or_default() += size * count,
            _ => {}
        }
        index += len + 1;
    }
    reports
}

#[test]
fn descriptors_preserve_boot_keyboard_and_identify_ble_reports() {
    use kb_core::hid::{
        BRAILLE_BLE_REPORT_DESCRIPTOR, CONSUMER_REPORT_DESCRIPTOR, KEYBOARD_REPORT_DESCRIPTOR,
    };
    let keyboard = descriptor_reports(&KEYBOARD_REPORT_DESCRIPTOR);
    assert_eq!(keyboard.get(&(0, 0x80)), Some(&64));
    assert_eq!(keyboard.get(&(0, 0x90)), Some(&8));
    let consumer = descriptor_reports(&CONSUMER_REPORT_DESCRIPTOR);
    assert_eq!(consumer.len(), 1);
    assert_eq!(consumer.get(&(0, 0x80)), Some(&16));
    let combined = descriptor_reports(&BRAILLE_BLE_REPORT_DESCRIPTOR);
    assert_eq!(combined.len(), 3);
    assert_eq!(combined.get(&(1, 0x80)), Some(&64));
    assert_eq!(combined.get(&(1, 0x90)), Some(&8));
    assert_eq!(combined.get(&(2, 0x80)), Some(&16));
}
