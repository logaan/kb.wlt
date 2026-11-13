use kb_core::hid::KEYBOARD_REPORT_DESCRIPTOR;
use kb_core::pipeline::{Action, Key};
use kb_core::report_state::{ReportError, ReportState};

#[test]
fn descriptor_covers_the_complete_encoded_keyboard_usage_range() {
    let mut bytes = KEYBOARD_REPORT_DESCRIPTOR.as_slice();
    let mut page = 0;
    let mut logical_minimum = 0;
    let mut logical_maximum = 0;
    let mut usage_minimum = 0;
    let mut usage_maximum = 0;
    let mut report_size = 0;
    let mut report_count = 0;
    let mut input_bits = 0;
    let mut output_bits = 0;
    let mut arrays = 0;
    while let Some((&prefix, rest)) = bytes.split_first() {
        assert_ne!(prefix, 0xfe);
        let size = match prefix & 3 {
            3 => 4,
            size => size as usize,
        };
        let (data, rest) = rest.split_at(size);
        bytes = rest;
        let value = data.iter().enumerate().fold(0, |value, (shift, byte)| {
            value | ((*byte as u32) << (shift * 8))
        });
        match ((prefix >> 2) & 3, prefix >> 4) {
            (1, 0) => page = value,
            (1, 1) => logical_minimum = value,
            (1, 2) => logical_maximum = value,
            (1, 7) => report_size = value,
            (1, 9) => report_count = value,
            (2, 1) => usage_minimum = value,
            (2, 2) => usage_maximum = value,
            (0, 8) => {
                input_bits += report_size * report_count;
                if value & 3 == 0 {
                    arrays += 1;
                    assert_eq!(page, 7);
                    assert_eq!((report_size, report_count), (8, 6));
                    assert_eq!((logical_minimum, logical_maximum), (0, 255));
                    assert_eq!((usage_minimum, usage_maximum), (0, 255));
                }
                usage_minimum = 0;
                usage_maximum = 0;
            }
            (0, 9) => output_bits += report_size * report_count,
            _ => {}
        }
    }
    assert_eq!(arrays, 1);
    assert_eq!(input_bits, 64);
    assert_eq!(output_bits, 8);
}

#[test]
fn extended_function_keys_and_modifiers_encode_without_truncation() {
    let mut reports = ReportState::<8>::new();
    for usage in [104, 115, 225] {
        reports
            .apply(Action::Press(Key {
                usage,
                modifiers: 0,
            }))
            .unwrap();
    }
    assert_eq!(reports.report(), [2, 0, 104, 115, 0, 0, 0, 0]);
    for usage in 0..=3 {
        assert_eq!(
            reports.apply(Action::Press(Key {
                usage,
                modifiers: 0
            })),
            Err(ReportError::Usage)
        );
    }
}
