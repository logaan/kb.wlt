use crate::{Result, targets::TARGETS};
use kb_component_build::Contract;
use kb_component_runtime::{BrailleComponent, KeyboardComponent};
use kb_core::{
    matrix_link::braille_keys,
    pipeline::{Action, Input, KeyEvent, KeyState},
    reference::{Entry, LEADER},
    split::Layout,
};
use kb_display::reference::{dot_label, label};
use std::{fmt::Write, fs, path::Path};

fn dots(cell: u16) -> String {
    if cell == 0 {
        "-".into()
    } else {
        dot_label(cell).as_str().into()
    }
}

fn diagram(out: &mut String, rows: &[Vec<String>], half: usize) {
    let width = rows.iter().flatten().map(String::len).max().unwrap_or(1);
    for row in rows {
        let split = half.min(row.len() / 2);
        let hand = |keys: &[String]| {
            keys.iter()
                .map(|k| format!("[{k:^width$}]"))
                .collect::<String>()
        };
        writeln!(
            out,
            "{:>pad$}    {}",
            hand(&row[..split]),
            hand(&row[split..]),
            pad = half * (width + 2)
        )
        .unwrap();
    }
}

fn toucan_map() -> Result<String> {
    let keyboard = KeyboardComponent::new(include_bytes!(
        "../../clients/components/generated/toucan-keymap/keyboard.core.wasm"
    ))
    .map_err(|e| format!("keyboard: {e:?}"))?;
    let mut out = String::new();
    for (layer, title) in ["TYPING", "NAVIGATION (hold left outer thumb)", "BRAILLE"]
        .into_iter()
        .enumerate()
    {
        writeln!(out, "{title}\n").unwrap();
        let mut keys = Vec::new();
        for pos in 0..42 {
            let name = match (layer, pos) {
                (0 | 1, 36) => "Nav".into(),
                (1, 10) => "Braille".into(),
                (2, _) => dots(braille_keys(1 << pos)),
                _ => {
                    let event = |position| KeyEvent {
                        position,
                        state: KeyState::Pressed,
                        timestamp_ms: 0,
                    };
                    let events = if layer == 1 {
                        vec![event(36), event(pos)]
                    } else {
                        vec![event(pos)]
                    };
                    let input =
                        Input::<2>::with_events(0, &events).map_err(|e| format!("input: {e:?}"))?;
                    let step = keyboard
                        .process::<64, 2, 2>(&[], &input)
                        .map_err(|e| format!("keymap: {e:?}"))?;
                    let step = keyboard
                        .transform_output::<64, 2>(step.state.as_slice(), step.actions.as_slice())
                        .map_err(|e| format!("keymap output: {e:?}"))?;
                    match step.actions.as_slice() {
                        [Action::Press(key)] => label(Entry {
                            cell: 0,
                            packed: u32::from(key.usage) | (u32::from(key.modifiers) << 8),
                        })
                        .as_str()
                        .trim_matches('\'')
                        .to_string(),
                        [] => "-".into(),
                        actions => {
                            return Err(format!("unexpected position {pos}: {actions:?}").into());
                        }
                    }
                }
            };
            keys.push(if name.is_empty() { "'".into() } else { name });
        }
        diagram(
            &mut out,
            &keys.chunks(12).map(<[_]>::to_vec).collect::<Vec<_>>(),
            6,
        );
        out.push('\n');
    }
    out.push_str("Left outer thumb, then P : Typing <-> Braille\n");
    out.push_str(&menu_cells());
    Ok(out)
}

fn paintbrush_map() -> String {
    let mut out = String::from("BRAILLE\n\n");
    let rows = (0..2)
        .map(|row| {
            [Layout::Left, Layout::Right]
                .into_iter()
                .flat_map(|hand| (0..4).map(move |col| dots(hand.map(1 << (row * 4 + col)))))
                .collect()
        })
        .collect::<Vec<_>>();
    diagram(&mut out, &rows, 4);
    out.push_str("\n- : unassigned\nSpace : either inner bottom key\n");
    out
}

fn menu_cells() -> String {
    let mut text = String::from("Menu: hold Space\n");
    for row in 0..3 {
        let dot = |bit: usize| {
            if LEADER & (1u16 << bit) != 0 {
                'o'
            } else {
                '.'
            }
        };
        let pair = format!("{} {}", dot(row), dot(row + 3));
        writeln!(text, "  {pair}    {pair}").unwrap();
    }
    text.push_str("  1st    2nd   (release dots between cells; within 2 s)\n");
    text
}

fn braille_reference() -> Result<String> {
    let mut braille = BrailleComponent::new(include_bytes!(
        "../../clients/components/generated/braille.core.wasm"
    ))
    .map_err(|e| format!("braille: {e:?}"))?;
    let mut out = String::from("DOTS\n1 4\n2 5\n3 6\n7 8\n\n");
    out.push_str(&menu_cells());
    out.push_str("Second cell: q Main | r Reference | c Configuration | b Bindings | d Diagnostics\n\nMENU CONTROLS (hold Space)\n1 / 4   Previous / Next      3 / 6   Previous page / Next page\n7       Select               12      Back\n\nCOMPUTER 8\nDots             Output\n",
    );
    for cell in 1..=511 {
        let packed = braille
            .map_cell(cell)
            .map_err(|e| format!("cell {cell}: {e:?}"))?;
        if cell == LEADER || packed != 0 {
            writeln!(
                out,
                "{:<16} {}",
                dots(cell),
                if cell == LEADER {
                    "Menu prefix".into()
                } else {
                    label(Entry { cell, packed }).as_str().to_string()
                }
            )?;
        }
    }
    out.push_str("\nUEB GRADE 1\nDot sequence         Meaning\n");
    for rule in kb_core::ueb1::RULES {
        let seq = rule.cells[..rule.count as usize]
            .iter()
            .map(|c| dots(*c))
            .collect::<Vec<_>>()
            .join(" / ");
        writeln!(out, "{seq:<20} {}", rule.label)?;
    }
    out.push_str("\nUEB COMMANDS\nDots             Output\n");
    for cell in 1..=511 {
        if cell == LEADER {
            writeln!(out, "{:<16} Menu prefix", dots(cell))?;
        } else if cell >= 64 {
            let packed = kb_core::ueb1::map_cell(cell);
            if packed != 0 {
                writeln!(
                    out,
                    "{:<16} {}",
                    dots(cell),
                    label(Entry { cell, packed }).as_str()
                )?;
            }
        }
    }
    Ok(out)
}

pub fn generate(board: &str, images: &Path, output: &Path) -> Result {
    let (contract, keymap) = match board {
        "toucan" => (Contract::Keyboard, toucan_map()?),
        "paintbrush" => (Contract::Braille, paintbrush_map()),
        _ => return Err("expected toucan or paintbrush".into()),
    };
    fs::create_dir_all(output)?;
    let mut install = format!(
        "{} + PROSPECTOR   v{}\n\nBOARD              BOOTLOADER                 IMAGE\n",
        board.to_uppercase(),
        env!("CARGO_PKG_VERSION")
    );
    for t in TARGETS.iter().filter(|t| t.contract == contract) {
        let role = if t.name.contains("left") {
            "Left half"
        } else if t.name.contains("right") {
            "Right half"
        } else {
            "USB receiver"
        };
        let boot = if t.flash_origin == xtask::S140_V7_APPLICATION_START {
            "XIAO S140 v7"
        } else {
            "nice!nano S140 v6"
        };
        let image = format!("{}.uf2", t.name);
        writeln!(
            install,
            "{role:<18} {boot:<16} {:#x}  {image}",
            t.flash_origin
        )?;
        fs::copy(images.join(&image), output.join(image))?;
    }
    install.push_str("\nINSTALL   one board at a time\n1. Connect board by USB; double-tap Reset to enter the UF2 bootloader.\n2. Open the mounted drive. Check INFO_UF2.TXT and bootloader against table.\n   Wrong board or bootloader: stop.\n3. Copy the matching .uf2 onto the drive. Wait for automatic reboot.\n   macOS/Linux: bash flash-uf2.sh IMAGE.uf2 MOUNTED_DRIVE\n4. Repeat for the other boards.\n\nRUN\nProspector -> USB computer\nLeft + right -> battery or USB power -> automatic pairing to Prospector\n\nREFERENCE\nKEYMAP.txt    Physical layouts (left | right, top to bottom)\nBRAILLE.txt   Menu, Computer 8, UEB Grade 1\n");
    fs::write(output.join("INSTALL.txt"), install)?;
    fs::write(output.join("KEYMAP.txt"), keymap)?;
    fs::write(output.join("BRAILLE.txt"), braille_reference()?)?;
    fs::write(
        output.join("flash-uf2.sh"),
        include_bytes!("../../scripts/flash-uf2.sh"),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn references_use_shipping_components_and_board_maps() {
        let toucan = toucan_map().unwrap();
        let paintbrush = paintbrush_map();
        let reference = braille_reference().unwrap();
        assert!(toucan.is_ascii() && paintbrush.is_ascii() && reference.is_ascii());
        for name in ["NAVIGATION", "Left Ctrl", "Backspace", "Braille", "Space"] {
            assert!(toucan.contains(name), "{name}");
        }
        assert!(
            paintbrush
                .lines()
                .nth(2)
                .unwrap()
                .split_whitespace()
                .collect::<String>()
                .contains("[7][3][2][1][4][5][6][8]")
        );
        for expected in [
            "12345+Space      Menu prefix",
            "1                'a'",
            "UEB GRADE 1",
            "6 / 1                A",
        ] {
            assert!(reference.contains(expected), "{expected}");
        }
    }
}
