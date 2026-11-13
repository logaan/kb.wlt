use crate::{S140_V7_APPLICATION_START, STOCK_APPLICATION_START};
use kb_component_build::Contract;

#[derive(Clone, Copy, Debug)]
pub struct Target {
    pub name: &'static str,
    pub binary: &'static str,
    pub features: &'static str,
    pub contract: Contract,
    pub flash_origin: u32,
}

impl Target {
    pub fn source(self) -> &'static str {
        match self.contract {
            Contract::Braille => "clients/components/braille/braille.wlt",
            Contract::Keyboard => "clients/components/toucan-keymap/toucan-keymap.wlt",
        }
    }
}

pub const TARGETS: &[Target] = &[
    Target {
        name: "toucan-left-satellite-display",
        binary: "toucan-satellite",
        features: "toucan,nice-view",
        contract: Contract::Keyboard,
        flash_origin: S140_V7_APPLICATION_START,
    },
    Target {
        name: "toucan-right-satellite",
        binary: "toucan-satellite",
        features: "toucan,toucan-right",
        contract: Contract::Keyboard,
        flash_origin: S140_V7_APPLICATION_START,
    },
    Target {
        name: "toucan-prospector",
        binary: "toucan-dongle",
        features: "toucan-dongle,usb,prospector-display",
        contract: Contract::Keyboard,
        flash_origin: S140_V7_APPLICATION_START,
    },
    Target {
        name: "paintbrush-left-satellite",
        binary: "paintbrush-satellite",
        features: "split,satellite-left",
        contract: Contract::Braille,
        flash_origin: STOCK_APPLICATION_START,
    },
    Target {
        name: "paintbrush-right-satellite",
        binary: "paintbrush-satellite",
        features: "split,satellite-right",
        contract: Contract::Braille,
        flash_origin: STOCK_APPLICATION_START,
    },
    Target {
        name: "prospector-pair",
        binary: "prospector-dongle",
        features: "usb,split,prospector-display",
        contract: Contract::Braille,
        flash_origin: S140_V7_APPLICATION_START,
    },
];

pub fn verify_release_names(names: &[String]) -> Result<(), String> {
    let expected = TARGETS
        .iter()
        .map(|target| format!("{}.uf2", target.name))
        .collect::<std::collections::BTreeSet<_>>();
    let actual = names
        .iter()
        .filter(|name| name.ends_with(".uf2"))
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "release UF2 mismatch: expected {expected:?}, found {actual:?}"
        ))
    }
}

pub fn find(name: &str) -> Option<Target> {
    TARGETS.iter().copied().find(|target| target.name == name)
}
