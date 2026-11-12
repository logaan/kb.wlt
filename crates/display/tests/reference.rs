use core::convert::Infallible;
use core::fmt::Write;
use kb_core::{
    KeyStroke, computer8,
    reference::{Catalog, Cursor, Entry, Mode, Page},
};
use kb_display::reference::{ACCENT, BACKGROUND, Label, dot_label, label, render_row};

fn entry(cell: u16, usage: u8, modifiers: u8) -> Entry {
    Entry {
        cell,
        packed: KeyStroke::new(usage, modifiers).packed(),
    }
}

#[test]
fn labels_cover_printable_ascii_commands_and_unknown_custom_outputs() {
    for ascii in 33..=126 {
        let key = KeyStroke::from_ascii(ascii).unwrap();
        assert_eq!(
            label(entry(1, key.usage, key.modifiers)).as_str(),
            format!("'{}'", char::from(ascii))
        );
    }
    for (usage, text) in [
        (40, "Return"),
        (41, "Escape"),
        (42, "Backspace"),
        (43, "Tab"),
        (44, "Space"),
        (74, "Home"),
        (75, "PageUp"),
        (78, "PageDown"),
        (79, "Right"),
        (80, "Left"),
        (81, "Down"),
        (82, "Up"),
    ] {
        assert_eq!(label(entry(1, usage, 0)).as_str(), text);
    }
    assert_eq!(label(entry(1, 4, 1)).as_str(), "Ctrl+'a'");
    assert_eq!(label(entry(1, 4, 0x20)).as_str(), "'A'");
    assert_eq!(label(entry(1, 0xf0, 0)).as_str(), "HID F0");
    assert_eq!(
        label(Entry {
            cell: 1,
            packed: 0x12345678
        })
        .as_str(),
        "Raw 12345678"
    );
    assert_eq!(label(entry(1, 0, 1)).as_str(), "Ctrl+Modifiers");
}

#[test]
fn numeric_dot_labels_distinguish_space_from_dot_eight() {
    assert_eq!(dot_label(0x3f).as_str(), "123456");
    assert_eq!(dot_label(0x80).as_str(), "8");
    assert_eq!(dot_label(0x140).as_str(), "7+Space");
    assert_eq!(dot_label(0x100).as_str(), "Space");
    let mut bounded = Label::new();
    assert!(bounded.write_str(&"x".repeat(49)).is_err());
    assert!(bounded.as_str().is_empty());
}

#[test]
fn renderer_draws_fourth_dot_row_only_for_eight_dot_cells() {
    let mut page = Page {
        mode: Mode::Reference,
        len: 1,
        total: 1,
        ..Page::default()
    };
    let mut row = [0; 480];
    page.entries[0] = entry(0x7f, 4, 2);
    render_row(&page, 73, &mut row);
    assert_eq!(&row[28..30], &ACCENT.to_be_bytes());
    page.entries[0].cell = 0x3f;
    render_row(&page, 73, &mut row);
    assert_ne!(&row[28..30], &ACCENT.to_be_bytes());
    render_row(&Page::default(), 279, &mut row);
    assert!(
        row.chunks_exact(2)
            .all(|pixel| pixel == BACKGROUND.to_be_bytes())
    );
}

#[test]
fn catalog_pages_and_namespace_render_full_frames() {
    let catalog = Catalog::try_from_fn(|cell| {
        Ok::<_, Infallible>(computer8(cell).map_or(0, |key| key.packed()))
    })
    .unwrap();
    let mut row = [0; 480];
    for mode in [Mode::Namespace, Mode::Reference, Mode::Find] {
        let page = Cursor::default().page(&catalog, mode);
        let mut changed = 0;
        for y in 0..280 {
            render_row(&page, y, &mut row);
            changed += row
                .chunks_exact(2)
                .filter(|pixel| *pixel != BACKGROUND.to_be_bytes())
                .count();
        }
        assert!(changed > 1000);
    }
}
