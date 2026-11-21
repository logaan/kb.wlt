use crate::{Result, targets::TARGETS};
use kb_client_session::labels::label;
use kb_component_build::Contract;
use kb_component_runtime::{BrailleComponent, KeyboardComponent};
use kb_core::{
    matrix_link::{BRAILLE_THUMBS, braille_keys},
    pipeline::{Action, Input, KeyEvent, KeyState},
    reference::{Entry, LEADER},
    split::Layout,
};
use std::{fmt::Write, fs, path::Path};

fn dots(cell: u16) -> String {
    match cell {
        0 => "-".into(),
        256 => "Space".into(),
        _ => {
            let glyph = char::from_u32(0x2800 + u32::from(cell & 255)).unwrap();
            if cell & 256 != 0 {
                format!("{glyph}\u{0332}")
            } else {
                glyph.to_string()
            }
        }
    }
}

fn diagram(out: &mut String, rows: &[Vec<String>], half: usize) {
    let width = rows
        .iter()
        .flatten()
        .map(|s| s.chars().count())
        .max()
        .unwrap_or(1);
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
    for (braille, title) in [(false, "TYPING"), (true, "BRAILLE")] {
        writeln!(out, "{title}\n").unwrap();
        let mut keys = Vec::new();
        for pos in 0..42 {
            let name = if (1 << pos) & BRAILLE_THUMBS != 0 {
                "Braille".into()
            } else if braille {
                dots(braille_keys(1 << pos))
            } else {
                let input = Input::<1>::with_events(
                    0,
                    &[KeyEvent {
                        position: pos,
                        state: KeyState::Pressed,
                        timestamp_ms: 0,
                    }],
                )
                .map_err(|e| format!("input: {e:?}"))?;
                let step = keyboard
                    .process::<64, 1, 1>(&[], &input)
                    .map_err(|e| format!("keymap: {e:?}"))?;
                let step = keyboard
                    .transform_output::<64, 1>(step.state.as_slice(), step.actions.as_slice())
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
    out.push_str(
        "Tap either outer thumb to toggle Braille. Hold either for momentary Braille.\n\
         Either inner thumb is Space in Braille. Each half has all eight dots.\n\
         A dot held on both halves releases when both keys are up.\n\n",
    );
    out.push_str(&menu_cells());
    out.push_str(TOUCAN_TARGET_MENU);
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
    format!(
        "Braille menu  {0}\nMain Menu     {0} {0}\nKeep Space held and release the other keys between cells.\n",
        dots(LEADER)
    )
}

const TOUCAN_TARGET_MENU: &str =
    "Target        ⠞   Choose Prospector, BT 1–5, or USB from the Braille menu.\n";

fn braille_reference(toucan: bool) -> Result<String> {
    let mut braille = BrailleComponent::new(include_bytes!(
        "../../clients/components/generated/braille.core.wasm"
    ))
    .map_err(|e| format!("braille: {e:?}"))?;
    let mut out = String::from("BRAILLE REFERENCE\nAn underlined cell includes Space.\n\n");
    out.push_str(&menu_cells());
    if toucan {
        out.push_str(TOUCAN_TARGET_MENU);
    }
    out.push_str("Menu navigation\n");
    for (cell, action) in [
        (0x101, "Previous"),
        (0x108, "Next"),
        (0x104, "Up"),
        (0x120, "Down"),
        (0x107, "First"),
        (0x138, "Last"),
        (0x13c, "Page up"),
        (0x139, "Page down"),
        (0x140, "Open"),
        (0x103, "Back"),
        (0x013, "Help"),
        (0x113, "Help while editing a binding"),
    ] {
        writeln!(out, "{:<16} {action}", dots(cell))?;
    }
    out.push_str("\nCOMPUTER 8\nInput            Output\n");
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
    out.push_str("\nUEB GRADE 1\nInput                Meaning\n");
    for rule in kb_core::ueb1::RULES {
        let seq = rule.cells[..rule.count as usize]
            .iter()
            .map(|c| dots(*c))
            .collect::<Vec<_>>()
            .join(" / ");
        writeln!(out, "{seq:<20} {}", rule.label)?;
    }
    out.push_str("\nUEB COMMANDS\nInput            Output\n");
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
        "{} INSTALL KIT   v{}\n\nBOARD              BOOTLOADER                 IMAGE\n",
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
    install.push_str(
        "\nINSTALL\nUse the matching images from this kit for all three boards on the first upgrade.\n\n\
         1. Connect one board by USB and double-tap Reset to enter its UF2 bootloader.\n\
         2. Check INFO_UF2.TXT on the mounted drive against the board and bootloader above.\n\
         3. Copy the matching UF2 to the drive and wait for the board to reboot.\n\
            On macOS/Linux you can run bash flash IMAGE.uf2 MOUNTED_DRIVE.\n\
         4. Repeat for the other boards.\n\nFIRST CONNECTION\n",
    );
    install.push_str(match contract {
        Contract::Keyboard => {
            "Connect Prospector to the computer over USB and power both halves.\n\
             The right half pairs with the left. The left pairs with Prospector.\n\
             Fresh installs type to kb.wlt Toucan Receiver over Prospector USB.\n\n\
             CHOOSE A HOST\n\
             Tap either outer thumb for Braille. Hold either inner thumb (Space)\n\
             and chord ⠟. Chord t (⠞) in the menu to open Target. Choose Prospector,\n\
             BT 1–5, or USB. The left half remembers the choice. Both displays show the\n\
             selected target and each half’s measured battery estimate when available.\n\
             For direct Bluetooth, choose an empty slot and pair kb.wlt Toucan.\n\
             If an old host keeps reconnecting during pairing, temporarily turn\n\
             Bluetooth off on that host. For USB, connect the left half to the\n\
             computer by data cable. Its device name is kb.wlt Toucan USB.\n\n\
             SUBSEQUENT UPDATES\n\
             Connect the target boards with USB data cables. In Braille mode, open\n\
             Main Menu > Debug > UF2 Bootloader, select the available boards, and\n\
             choose Enter UF2. The right half and Prospector must be linked to\n\
             receive a command from Toucan. A touch-wired Prospector can enter its\n\
             own bootloader from the same menu while disconnected.\n"
        }
        Contract::Braille => {
            "Connect Prospector to the computer over USB and power both halves.\n\
             The halves pair automatically with the Prospector.\n\
             The computer sees kb.wlt Split Receiver as its USB keyboard.\n\n\
             SUBSEQUENT UPDATES\n\
             With Prospector on USB, open Main Menu > Debug > UF2 Bootloader,\n\
             select Prospector, and choose Enter UF2. Use Reset on the Paintbrush halves.\n"
        }
    });
    install.push_str(
        "\nPROSPECTOR TOUCH\n\
         The same firmware runs on boards wired only for the LCD. Touch requires\n\
         the CST816 panel wired to the XIAO (TP_IRQ P0.02, TP_RST P0.03, TP_SDA\n\
         P0.04, TP_SCL P0.05). Tap to open, swipe to navigate, and hold to go back.\n",
    );
    install.push_str(
        "\nREFERENCE\nKEYMAP.txt    Physical layouts (left | right, top to bottom)\n\
         BRAILLE.txt   Menu navigation, Computer 8, UEB Grade 1\n",
    );
    fs::write(output.join("INSTALL.txt"), install)?;
    fs::write(output.join("KEYMAP.txt"), keymap)?;
    fs::write(
        output.join("BRAILLE.txt"),
        braille_reference(contract == Contract::Keyboard)?,
    )?;
    fs::write(
        output.join("LICENSES.txt"),
        concat!(
            "LVGL 9.2.2\n",
            include_str!("../../shared/display/lvgl/LICENSE"),
            "\nMontserrat\n",
            include_str!("../../shared/display/lvgl/OFL-Montserrat.txt"),
            "\nWABT 1.0.42\n",
            include_str!("../../host/runtime/embedded/LICENSE-wabt")
        ),
    )?;
    fs::write(output.join("flash"), include_bytes!("../../scripts/flash"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn references_use_shipping_components_and_board_maps() {
        let toucan = toucan_map().unwrap();
        let paintbrush = paintbrush_map();
        let reference = braille_reference(true).unwrap();
        assert!(
            [&toucan, &paintbrush, &reference]
                .iter()
                .all(|text| text.contains('⠁'))
        );
        for name in ["Left Ctrl", "Backspace", "Braille", "Space"] {
            assert!(toucan.contains(name), "{name}");
        }
        let braille = toucan.split_once("BRAILLE\n\n").unwrap().1;
        for (row, expected) in [
            (1, "[-][⡀][⠄][⠂][⠁][-][-][⠈][⠐][⠠][⢀][-]"),
            (2, "[-][⢀][⠠][⠐][⠈][-][-][⠁][⠂][⠄][⡀][-]"),
            (3, "[Braille][-][Space][Space][-][Braille]"),
        ] {
            let actual = braille.lines().nth(row).unwrap();
            assert_eq!(actual.split_whitespace().collect::<String>(), expected);
        }
        assert!(toucan.contains(TOUCAN_TARGET_MENU));
        assert!(reference.contains(TOUCAN_TARGET_MENU));
        assert!(
            !braille_reference(false)
                .unwrap()
                .contains(TOUCAN_TARGET_MENU)
        );
        assert!(
            paintbrush
                .lines()
                .nth(2)
                .unwrap()
                .split_whitespace()
                .collect::<String>()
                .contains("[⡀][⠄][⠂][⠁][⠈][⠐][⠠][⢀]")
        );
        for expected in [
            "⠟̲               Menu prefix",
            "⠁                'a'",
            "UEB GRADE 1",
            "⠠ / ⠁                A",
        ] {
            assert!(reference.contains(expected), "{expected}");
        }
    }
}
