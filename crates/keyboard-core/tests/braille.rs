use kb_core::braille::{COMPUTER8_ASCII_CELLS, GLOBE_USAGE, ascii_cell};
use kb_core::hid::LEFT_SHIFT;
use kb_core::{KeyStroke, computer8};

#[test]
fn printable_ascii_has_unique_complete_braille_and_hid_mappings() {
    let mut seen = [false; 256];
    for ascii in b'!'..=b'~' {
        let cell = ascii_cell(ascii).unwrap();
        assert!(!seen[cell as usize]);
        seen[cell as usize] = true;
        assert_eq!(computer8(cell), KeyStroke::from_ascii(ascii));
    }
    assert_eq!(seen.into_iter().filter(|seen| *seen).count(), 94);
    assert_eq!(COMPUTER8_ASCII_CELLS.len(), 94);
}

#[test]
fn capitalization_uses_dot_seven_and_shift() {
    for ascii in b'a'..=b'z' {
        let cell = ascii_cell(ascii).unwrap();
        let lower = computer8(cell).unwrap();
        let upper = computer8(cell | 0x40).unwrap();
        assert_eq!(upper.usage, lower.usage);
        assert_eq!(upper.modifiers, LEFT_SHIFT);
        assert_eq!(lower.modifiers, 0);
    }
}

#[test]
fn hello_world_matches_reference_computer_braille_cells() {
    let cells = [
        0x53, 0x11, 0x07, 0x07, 0x15, 0x20, 0x100, 0x7a, 0x15, 0x17, 0x07, 0x19, 0x2e,
    ];
    for (&cell, &ascii) in cells.iter().zip(b"Hello, World!") {
        assert_eq!(computer8(cell), KeyStroke::from_ascii(ascii));
    }
}

#[test]
fn eight_key_commands_and_reserved_cells_are_distinct() {
    assert_eq!(computer8(0x40), Some(KeyStroke::new(42, 0)));
    assert_eq!(computer8(0x80), Some(KeyStroke::new(40, 0)));
    assert_eq!(computer8(0x100), Some(KeyStroke::new(44, 0)));
    assert_eq!(computer8(0x140), Some(KeyStroke::new(40, 0)));
    assert_eq!(computer8(0xc0), None);
    for cell in 0..=255 {
        if cell & 0x80 != 0 && cell != 0x80 {
            assert_eq!(computer8(cell), None, "{cell:#x}");
        }
    }
    assert_eq!(computer8(0), None);
    assert_eq!(computer8(0x200), None);
    assert_eq!(computer8(u16::MAX), None);
}

#[test]
fn one_shot_commands_have_tagged_keyboard_or_consumer_usage() {
    for (cell, modifiers, consumer) in [
        (0x148, 2, 0),
        (0x144, 1, 0),
        (0x142, 4, 0),
        (0x141, 8, 0),
        (0x150, 0, GLOBE_USAGE),
    ] {
        let stroke = KeyStroke::new(0, modifiers)
            .with_consumer(consumer)
            .sticky();
        assert_eq!(computer8(cell), Some(stroke));
        assert_eq!(kb_core::ueb1::map_cell(cell), stroke.packed());
        assert_eq!(KeyStroke::from_packed(stroke.packed()), Some(stroke));
    }
}

#[test]
fn underscore_and_brackets_follow_the_existing_ebae_table() {
    for (cell, ascii) in [
        (0x38, b'_'),
        (0x6a, b'['),
        (0x73, b'\\'),
        (0x7b, b']'),
        (0x2a, b'{'),
    ] {
        assert_eq!(computer8(cell), KeyStroke::from_ascii(ascii));
    }
}
