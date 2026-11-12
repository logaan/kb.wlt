use crate::reference::{
    ACCENT, BACKGROUND, Label, MUTED, Row, SELECTED, SPACE_COLOR, WHITE, cell, escaped, text,
};
use core::fmt::Write;
use kb_core::{
    menu::{Diagnostics, State},
    preferences::Settings,
    reference::{Entry, Mode},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Page {
    pub mode: Mode,
    pub state: State,
    pub settings: Settings,
    pub diagnostics: Diagnostics,
}

pub fn render_row(page: &Page, y: usize, bytes: &mut [u8; 480]) {
    let list = matches!(page.mode, Mode::Main | Mode::Config | Mode::BindingOptions);
    let selected_top = 48 + usize::from(page.state.selected) * 29;
    let binding_top = 48 + usize::from(page.state.selected % 4) * 46;
    let highlighted = (list && (selected_top..selected_top + 28).contains(&y))
        || (page.mode == Mode::Bindings && (binding_top..binding_top + 45).contains(&y));
    let background = if highlighted { SELECTED } else { BACKGROUND };
    for pixel in bytes.chunks_exact_mut(2) {
        pixel.copy_from_slice(&background.to_be_bytes());
    }
    let mut row = Row { y, bytes };
    let title = match page.mode {
        Mode::Main => "QUICK BRAILLE FOX",
        Mode::Config => "CONFIGURATION",
        Mode::Bindings => "CHORDS / CONTRACTIONS",
        Mode::Diagnostics => "DIAGNOSTICS",
        Mode::BindingOptions => "EDIT BINDING",
        Mode::BindingCapture => "CAPTURE TRIGGER",
        Mode::BindingText => "TYPE EXPANSION",
        _ => "QUICK BRAILLE FOX",
    };
    text(&mut row, title, 10, 18, ACCENT, true);
    let mut subtitle = Label::new();
    if matches!(
        page.mode,
        Mode::BindingOptions | Mode::BindingCapture | Mode::BindingText
    ) {
        let _ = write!(subtitle, "Slot {} / 12", page.state.slot + 1);
    } else {
        let _ = subtitle.write_str(page.settings.mode.name());
        if page.diagnostics.settings_unsaved {
            let _ = subtitle.write_str(" / UNSAVED");
        }
    }
    text(&mut row, subtitle.as_str(), 10, 35, MUTED, false);
    match page.mode {
        Mode::Main => {
            for (index, label) in [
                "qr  Braille reference",
                "qc  Configuration",
                "qb  Chords / contractions",
                "qd  Diagnostics",
            ]
            .iter()
            .enumerate()
            {
                text(&mut row, label, 14, 65 + index as i32 * 29, WHITE, false);
            }
            text(
                &mut row,
                "Hold Space for q then q/r/c/b/d.",
                10,
                193,
                MUTED,
                false,
            );
        }
        Mode::Config => {
            let mut mode = Label::new();
            let _ = write!(mode, "Braille: {}", page.settings.mode.name());
            for (index, label) in [
                mode.as_str(),
                "Restart keyboard links",
                "Clear pairings and pair again",
            ]
            .iter()
            .enumerate()
            {
                text(&mut row, label, 14, 65 + index as i32 * 29, WHITE, false);
            }
            text(
                &mut row,
                "Select braille to switch modes.",
                10,
                193,
                MUTED,
                false,
            );
            text(
                &mut row,
                "Radio actions reconnect input.",
                10,
                213,
                MUTED,
                false,
            );
        }
        Mode::BindingOptions => {
            for (index, label) in [
                "1 chord trigger",
                "2 chord trigger",
                "3 chord trigger",
                "4 chord trigger",
                "Edit expansion",
                "Delete this binding",
            ]
            .iter()
            .enumerate()
            {
                text(&mut row, label, 14, 65 + index as i32 * 29, WHITE, false);
            }
        }
        Mode::Bindings => {
            let start = usize::from(page.state.selected) / 4 * 4;
            for (offset, binding) in page
                .settings
                .bindings
                .iter()
                .skip(start)
                .take(4)
                .enumerate()
            {
                let top = 51 + offset as i32 * 46;
                for (index, trigger) in binding.trigger().iter().enumerate() {
                    cell(
                        &mut row,
                        Entry {
                            cell: *trigger,
                            packed: 0,
                        },
                        12 + index as i32 * 23,
                        top,
                    );
                    if trigger & 0x100 != 0 {
                        text(
                            &mut row,
                            "S",
                            14 + index as i32 * 23,
                            top + 34,
                            SPACE_COLOR,
                            false,
                        );
                    }
                }
                let mut label = Label::new();
                let _ = write!(
                    label,
                    "Slot {} {}",
                    start + offset + 1,
                    if binding.is_empty() { "empty" } else { "" }
                );
                text(&mut row, label.as_str(), 112, top + 10, WHITE, false);
                let output = escaped(binding.text());
                text(
                    &mut row,
                    output.as_str().get(..34).unwrap_or(output.as_str()),
                    12,
                    top + 41,
                    MUTED,
                    false,
                );
            }
        }
        Mode::BindingCapture => {
            let mut prompt = Label::new();
            let _ = write!(
                prompt,
                "Enter chord {} of {}",
                page.state.captured + 1,
                page.state.capture_count
            );
            text(&mut row, prompt.as_str(), 10, 65, WHITE, true);
            for (index, trigger) in page.state.draft.trigger().iter().enumerate() {
                cell(
                    &mut row,
                    Entry {
                        cell: *trigger,
                        packed: 0,
                    },
                    18 + index as i32 * 54,
                    88,
                );
                let x = 12 + index as i32 * 54;
                let label = crate::reference::dot_label(trigger & 0xff);
                text(&mut row, label.as_str(), x, 130, MUTED, false);
                if trigger & kb_core::reference::SPACE != 0 {
                    text(&mut row, "Space", x, 146, SPACE_COLOR, false);
                }
            }
            text(
                &mut row,
                "Press each chord, then release.",
                10,
                171,
                MUTED,
                false,
            );
            text(
                &mut row,
                "Space+q remains reserved.",
                10,
                193,
                SPACE_COLOR,
                false,
            );
            text(
                &mut row,
                "Space+q then Space+q cancels.",
                10,
                215,
                MUTED,
                false,
            );
        }
        Mode::BindingText => {
            let mut prompt = Label::new();
            let _ = write!(
                prompt,
                "Computer 8 editor: {}/24",
                page.state.draft.text_len
            );
            text(&mut row, prompt.as_str(), 10, 63, WHITE, false);
            let output = escaped(page.state.draft.text());
            let first = output.as_str().get(..32).unwrap_or(output.as_str());
            text(&mut row, first, 10, 96, ACCENT, false);
            if let Some(rest) = output.as_str().get(32..) {
                text(&mut row, rest, 10, 114, ACCENT, false);
            }
            text(
                &mut row,
                "Type the replacement text.",
                10,
                154,
                MUTED,
                false,
            );
            text(
                &mut row,
                "Dot7: erase  Space+7: save",
                10,
                177,
                MUTED,
                false,
            );
            text(&mut row, "Space+12: cancel", 10, 199, MUTED, false);
        }
        Mode::Diagnostics => {
            let diagnostics = page.diagnostics;
            for index in 0..6 {
                let mut label = Label::new();
                match index {
                    0 => {
                        let _ = write!(
                            label,
                            "USB {}  Bluetooth peers {}",
                            if diagnostics.usb_ready {
                                "ready"
                            } else {
                                "waiting"
                            },
                            diagnostics.peers
                        );
                    }
                    1 => {
                        let _ = write!(label, "Uptime {} seconds", diagnostics.uptime_seconds);
                    }
                    2 => {
                        let _ = write!(label, "Held dots {:03X}", diagnostics.held);
                    }
                    3 => {
                        let _ = write!(
                            label,
                            "Last HID {:02X}:{:02X}",
                            diagnostics.last_modifiers, diagnostics.last_usage
                        );
                    }
                    4 => {
                        let _ = write!(
                            label,
                            "Component: {}",
                            if diagnostics.component_fault {
                                "FAULT"
                            } else {
                                "running"
                            }
                        );
                    }
                    _ => {
                        let _ = write!(
                            label,
                            "Storage: {}",
                            if diagnostics.pairing_unsaved || diagnostics.settings_unsaved {
                                "UNSAVED"
                            } else {
                                "saved"
                            }
                        );
                    }
                }
                text(&mut row, label.as_str(), 12, 65 + index * 26, WHITE, false);
            }
        }
        _ => {}
    }
    if page.state.notice != kb_core::menu::Notice::None {
        text(
            &mut row,
            page.state.notice.text(),
            10,
            244,
            SPACE_COLOR,
            false,
        );
    } else if page.diagnostics.settings_unsaved {
        text(
            &mut row,
            "Settings unsaved; active now",
            10,
            244,
            SPACE_COLOR,
            false,
        );
    }
    if !matches!(page.mode, Mode::BindingCapture | Mode::BindingText) {
        if page.mode != Mode::Diagnostics {
            text(
                &mut row,
                "Space+1/4:move  Space+7:select",
                10,
                260,
                MUTED,
                false,
            );
        }
        text(
            &mut row,
            "Space+12:back  Space+q twice:home",
            10,
            274,
            MUTED,
            false,
        );
    }
}
