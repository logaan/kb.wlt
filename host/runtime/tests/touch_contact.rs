#[path = "../../../firmware/nrf52840/src/touch_contact.rs"]
mod touch_contact;

use kb_client_session::Navigation;
use touch_contact::Contact;

#[test]
fn each_contact_emits_at_most_one_action() {
    let mut contact = Contact::default();
    assert_eq!(contact.sample(0, Some([100, 100]), false), None);
    assert!(contact.active());
    assert_eq!(contact.sample(100, None, false), Some(Navigation::Enter));
    assert!(!contact.active());
    assert_eq!(contact.sample(200, Some([140, 130]), false), None);
    assert_eq!(
        contact.sample(750, Some([144, 131]), false),
        Some(Navigation::Escape)
    );
    assert_eq!(contact.sample(850, Some([144, 131]), false), None);
    assert_eq!(contact.sample(900, None, false), None);
    assert_eq!(contact.sample(901, Some([130, 140]), false), None);
    assert_eq!(contact.sample(945, None, false), Some(Navigation::Enter));
}

#[test]
fn swipes_follow_the_landscape_rotation() {
    for (point, expected) in [
        ([148, 100], Navigation::Up),
        ([95, 100], Navigation::Down),
        ([120, 145], Navigation::Right),
        ([120, 50], Navigation::Left),
    ] {
        let mut contact = Contact::default();
        assert_eq!(contact.sample(0, Some([120, 100]), true), None);
        assert_eq!(contact.sample(80, Some(point), true), None);
        assert_eq!(contact.sample(140, Some([120, 106]), true), None);
        assert_eq!(contact.sample(200, None, true), Some(expected));
    }
}

#[test]
fn a_link_change_during_contact_cancels_that_gesture_only() {
    let mut contact = Contact::default();
    assert_eq!(contact.sample(0, Some([90, 90]), true), None);
    assert_eq!(contact.sample(100, Some([94, 113]), false), None);
    assert_eq!(contact.sample(200, None, true), None);
    assert_eq!(contact.sample(300, Some([96, 94]), false), None);
    assert_eq!(contact.sample(350, None, false), Some(Navigation::Enter));
}
