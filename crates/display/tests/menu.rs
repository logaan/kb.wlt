use kb_core::reference::Mode;
use kb_display::{
    menu::{Page, render_row},
    reference::{ACCENT, BACKGROUND, SELECTED, SPACE_COLOR},
};

#[test]
fn every_menu_renders_its_heading_and_full_background() {
    let mut headings = Vec::new();
    for mode in [
        Mode::Main,
        Mode::Config,
        Mode::Bindings,
        Mode::BindingOptions,
        Mode::BindingCapture,
        Mode::BindingText,
        Mode::Diagnostics,
    ] {
        let page = Page {
            mode,
            ..Page::default()
        };
        let mut row = [0; 480];
        let mut heading = Vec::new();
        for y in 6..20 {
            render_row(&page, y, &mut row);
            heading.extend_from_slice(&row);
        }
        assert!(
            heading
                .chunks_exact(2)
                .any(|pixel| pixel == ACCENT.to_be_bytes())
        );
        assert!(!headings.contains(&heading));
        headings.push(heading);
        render_row(&page, 279, &mut row);
        assert!(
            row.chunks_exact(2)
                .all(|pixel| pixel == BACKGROUND.to_be_bytes())
        );
    }
}

#[test]
fn binding_list_draws_four_chords_and_space_marker_on_selected_slot() {
    let mut page = Page {
        mode: Mode::Bindings,
        ..Page::default()
    };
    page.state.selected = 6;
    page.settings
        .set_binding(6, &[1, 8, 64, 0x101], b"test")
        .unwrap();
    let mut row = [0; 480];
    let top = 51 + 2 * 46;
    render_row(&page, top, &mut row);
    assert_eq!(row[..2], SELECTED.to_be_bytes());
    render_row(&page, top + 1, &mut row);
    assert_eq!(row[166..168], ACCENT.to_be_bytes());
    render_row(&page, top + 29, &mut row);
    assert!(
        row[166..184]
            .chunks_exact(2)
            .any(|pixel| pixel == SPACE_COLOR.to_be_bytes())
    );
}
