use core::fmt::{self, Write};
use kb_core::{KeyStroke, braille::GLOBE_USAGE, reference::Entry};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Label {
    bytes: [u8; 48],
    len: usize,
}

impl Label {
    pub const fn new() -> Self {
        Self {
            bytes: [0; 48],
            len: 0,
        }
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).unwrap()
    }
}

impl Default for Label {
    fn default() -> Self {
        Self::new()
    }
}

impl Write for Label {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if self.len + text.len() > self.bytes.len() {
            return Err(fmt::Error);
        }
        self.bytes[self.len..self.len + text.len()].copy_from_slice(text.as_bytes());
        self.len += text.len();
        Ok(())
    }
}

pub(crate) fn escaped(bytes: &[u8]) -> Label {
    let mut label = Label::new();
    for byte in bytes {
        match byte {
            b'\n' => {
                let _ = label.write_str("\\n");
            }
            b'\t' => {
                let _ = label.write_str("\\t");
            }
            byte => {
                let _ = write!(label, "{}", char::from(*byte));
            }
        }
    }
    label
}

pub fn label(entry: Entry) -> Label {
    let mut output = Label::new();
    let stroke = entry.stroke();
    if stroke.is_sticky() {
        for (mask, name) in [
            (0x11, "Control"),
            (0x22, "Shift"),
            (0x44, "Alt"),
            (0x88, "Command"),
        ] {
            if stroke.modifiers & mask != 0 {
                if !output.as_str().is_empty() {
                    let _ = output.write_str("+");
                }
                let _ = output.write_str(name);
            }
        }
        if stroke.consumer != 0 {
            if !output.as_str().is_empty() {
                let _ = output.write_str("+");
            }
            if stroke.consumer == GLOBE_USAGE {
                let _ = output.write_str("Globe");
            } else {
                let _ = output.write_str("Media key");
            }
        }
        let _ = output.write_str(" one-shot");
        return output;
    }
    if stroke.consumer != 0 {
        let _ = output.write_str(if stroke.consumer == GLOBE_USAGE {
            "Globe"
        } else {
            "Media key"
        });
        if stroke.usage == 0 {
            return output;
        }
        let _ = output.write_str("+");
    }
    let shifted = stroke.modifiers & 0x22 != 0;
    for (mask, text) in [(0x11, "Ctrl+"), (0x44, "Alt+"), (0x88, "Command+")] {
        if stroke.modifiers & mask != 0 {
            let _ = output.write_str(text);
        }
    }
    let normalized = KeyStroke::new(stroke.usage, if shifted { 2 } else { 0 });
    if let Some(ascii) = (33..=126).find(|ascii| KeyStroke::from_ascii(*ascii) == Some(normalized))
    {
        let _ = write!(output, "'{}'", char::from(ascii));
        return output;
    }
    if shifted {
        let _ = output.write_str("Shift+");
    }
    let name = match stroke.usage {
        0 => "Modifiers",
        40 => "Return",
        41 => "Escape",
        42 => "Backspace",
        43 => "Tab",
        44 => "Space",
        57 => "CapsLock",
        70 => "PrintScreen",
        71 => "ScrollLock",
        72 => "Pause",
        73 => "Insert",
        74 => "Home",
        75 => "PageUp",
        76 => "Delete",
        77 => "End",
        78 => "PageDown",
        79 => "Right",
        80 => "Left",
        81 => "Down",
        82 => "Up",
        83 => "NumLock",
        84 => "Keypad /",
        85 => "Keypad *",
        86 => "Keypad -",
        87 => "Keypad +",
        88 => "Keypad Enter",
        98 => "Keypad 0",
        99 => "Keypad .",
        101 => "Menu",
        102 => "Power",
        103 => "Keypad =",
        224 => "Left Ctrl",
        225 => "Left Shift",
        226 => "Left Alt",
        227 => "Left Command",
        228 => "Right Ctrl",
        229 => "Right Shift",
        230 => "Right Alt",
        231 => "Right Command",
        _ => "",
    };
    if !name.is_empty() {
        let _ = output.write_str(name);
    } else if (58..=69).contains(&stroke.usage) {
        let _ = write!(output, "F{}", stroke.usage - 57);
    } else if (104..=115).contains(&stroke.usage) {
        let _ = write!(output, "F{}", stroke.usage - 91);
    } else if (89..=97).contains(&stroke.usage) {
        let _ = write!(output, "Keypad {}", stroke.usage - 88);
    } else {
        let _ = output.write_str("Special key");
    }
    output
}

pub fn release_label(strokes: &[KeyStroke], waiting: bool, leader: bool) -> Label {
    let mut result = Label::new();
    if leader {
        let _ = result.write_str("Menu prefix");
        return result;
    }
    for stroke in strokes.iter().copied() {
        if stroke.is_held() && stroke.consumer == 0 && stroke.modifiers == 0 {
            continue;
        }
        let stroke = if stroke.is_held() {
            KeyStroke::new(0, stroke.modifiers)
                .with_consumer(stroke.consumer)
                .sticky()
        } else {
            stroke
        };
        let name = label(Entry {
            cell: 0,
            packed: stroke.packed(),
        });
        let text = name.as_str();
        let short = if text.len() == 3 && text.starts_with('\'') {
            &text[1..2]
        } else if text == "Space" && strokes.len() > 1 {
            " "
        } else {
            text
        };
        if result.as_str().len() + short.len() > 39 {
            let _ = result.write_str("...");
            break;
        }
        let _ = result.write_str(short);
    }
    if result.as_str().is_empty() {
        let _ = result.write_str(if waiting {
            "Wait for next chord"
        } else {
            "No text"
        });
    }
    result
}
