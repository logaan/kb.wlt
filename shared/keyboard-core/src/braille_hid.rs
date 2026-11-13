use crate::KeyStroke;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Report {
    Keyboard([u8; 8]),
    Consumer([u8; 2]),
}

pub fn stroke_reports(previous_consumer: [u8; 2], stroke: KeyStroke) -> [Option<Report>; 4] {
    if stroke.is_sticky() {
        return [None; 4];
    }
    let keyboard = stroke.report();
    let consumer = stroke.consumer_report();
    let next_consumer = if stroke.is_held() { consumer } else { [0; 2] };
    [
        (consumer != [0; 2] && consumer != previous_consumer).then_some(Report::Consumer(consumer)),
        Some(Report::Keyboard(keyboard)),
        (!stroke.is_held() && keyboard != [0; 8]).then_some(Report::Keyboard([0; 8])),
        (next_consumer == [0; 2] && (previous_consumer != [0; 2] || consumer != [0; 2]))
            .then_some(Report::Consumer([0; 2])),
    ]
}

#[cfg(test)]
mod tests {
    use super::{Report, stroke_reports};
    use crate::KeyStroke;

    #[test]
    fn globe_tap_arms_consumer_before_keyboard_and_releases_it_after() {
        let stroke = KeyStroke::new(4, 2).with_consumer(0x29d);
        assert_eq!(
            stroke_reports([0; 2], stroke),
            [
                Some(Report::Consumer([0x9d, 0x02])),
                Some(Report::Keyboard([2, 0, 4, 0, 0, 0, 0, 0])),
                Some(Report::Keyboard([0; 8])),
                Some(Report::Consumer([0; 2])),
            ]
        );
    }

    #[test]
    fn held_reports_and_explicit_release_do_not_rearm_consumer() {
        let held = KeyStroke::new(0, 2).with_consumer(0x29d).held();
        assert_eq!(
            stroke_reports([0; 2], held),
            [
                Some(Report::Consumer([0x9d, 0x02])),
                Some(Report::Keyboard([2, 0, 0, 0, 0, 0, 0, 0])),
                None,
                None,
            ]
        );
        let tap = KeyStroke::new(4, 2).with_consumer(0x29d);
        assert_eq!(stroke_reports([0x9d, 0x02], tap)[0], None);
        assert_eq!(
            stroke_reports([0x9d, 0x02], KeyStroke::default().held()),
            [
                None,
                Some(Report::Keyboard([0; 8])),
                None,
                Some(Report::Consumer([0; 2])),
            ]
        );
        assert_eq!(
            stroke_reports([0; 2], KeyStroke::new(0, 2).sticky()),
            [None; 4]
        );
    }
}
