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
    for packed in [0, 1, 2, 3, 0x102, 0x10000, u32::MAX] {
        assert_eq!(KeyStroke::from_packed(packed), None);
    }
}
