use crate::{S140_V7_APPLICATION_START, STOCK_APPLICATION_START};
use kb_component_build::Contract;

#[derive(Clone, Copy, Debug)]
pub struct Target {
    pub name: &'static str,
    pub binary: &'static str,
    pub features: &'static str,
    pub contract: Option<Contract>,
    pub flash_origin: u32,
}

impl Target {
    const fn new(
        name: &'static str,
        binary: &'static str,
        features: &'static str,
        contract: Option<Contract>,
        flash_origin: u32,
    ) -> Self {
        Self {
            name,
            binary,
            features,
            contract,
            flash_origin,
        }
    }

    pub fn source(self) -> Option<&'static str> {
        match self.contract {
            Some(Contract::Led) => Some("components/led/led.wlt"),
            Some(Contract::Braille) => Some("components/braille/braille.wlt"),
            Some(Contract::Keyboard) => Some("components/toucan-keymap/toucan-keymap.wlt"),
            None => None,
        }
    }
}

const NANO: u32 = STOCK_APPLICATION_START;
const XIAO: u32 = S140_V7_APPLICATION_START;

pub const TARGETS: &[Target] = &[
    Target::new("xiao-led", "xiao-led", "usb", Some(Contract::Led), XIAO),
    Target::new(
        "xiao-led-composed",
        "xiao-led",
        "usb,composed",
        Some(Contract::Led),
        XIAO,
    ),
    Target::new(
        "paintbrush-usb",
        "paintbrush-usb",
        "usb",
        Some(Contract::Braille),
        NANO,
    ),
    Target::new(
        "paintbrush-usb-left",
        "paintbrush-usb",
        "usb,paintbrush-left",
        Some(Contract::Braille),
        NANO,
    ),
    Target::new(
        "paintbrush-ble",
        "paintbrush-ble",
        "ble",
        Some(Contract::Braille),
        NANO,
    ),
    Target::new(
        "paintbrush-ble-left",
        "paintbrush-ble",
        "ble,paintbrush-left",
        Some(Contract::Braille),
        NANO,
    ),
    Target::new(
        "paintbrush-display",
        "paintbrush-usb",
        "usb,display",
        Some(Contract::Braille),
        NANO,
    ),
    Target::new(
        "paintbrush-ble-display",
        "paintbrush-ble",
        "ble,display",
        Some(Contract::Braille),
        NANO,
    ),
    Target::new(
        "paintbrush-satellite",
        "paintbrush-satellite",
        "split",
        Some(Contract::Braille),
        NANO,
    ),
    Target::new(
        "paintbrush-left-satellite",
        "paintbrush-satellite",
        "split,satellite-left",
        Some(Contract::Braille),
        NANO,
    ),
    Target::new(
        "paintbrush-right-satellite",
        "paintbrush-satellite",
        "split,satellite-right",
        Some(Contract::Braille),
        NANO,
    ),
    Target::new(
        "paintbrush-central",
        "paintbrush-central",
        "usb,split",
        Some(Contract::Braille),
        NANO,
    ),
    Target::new(
        "prospector-single",
        "prospector-dongle",
        "usb,split,prospector-display",
        Some(Contract::Braille),
        XIAO,
    ),
    Target::new(
        "prospector-pair",
        "prospector-dongle",
        "usb,split,split-pair,prospector-display",
        Some(Contract::Braille),
        XIAO,
    ),
    Target::new(
        "settings-reset-nano",
        "settings-reset",
        "settings-reset",
        None,
        NANO,
    ),
    Target::new(
        "settings-reset-xiao",
        "settings-reset",
        "settings-reset",
        None,
        XIAO,
    ),
    Target::new(
        "toucan-ble",
        "toucan-ble",
        "toucan",
        Some(Contract::Keyboard),
        XIAO,
    ),
    Target::new(
        "toucan-ble-display",
        "toucan-ble",
        "toucan,nice-view",
        Some(Contract::Keyboard),
        XIAO,
    ),
    Target::new(
        "toucan-right-satellite",
        "toucan-satellite",
        "toucan,toucan-right",
        Some(Contract::Keyboard),
        XIAO,
    ),
    Target::new(
        "toucan-left-satellite",
        "toucan-satellite",
        "toucan",
        Some(Contract::Keyboard),
        XIAO,
    ),
    Target::new(
        "toucan-prospector",
        "toucan-dongle",
        "toucan-dongle,usb,prospector-display",
        Some(Contract::Keyboard),
        XIAO,
    ),
];

pub fn find(name: &str) -> Option<Target> {
    TARGETS.iter().copied().find(|target| target.name == name)
}
