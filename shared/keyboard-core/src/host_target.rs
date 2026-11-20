use crate::preferences::{ENCODED_LEN, Error, Settings};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum Target {
    #[default]
    Prospector = 0,
    Bluetooth1 = 1,
    Bluetooth2 = 2,
    Bluetooth3 = 3,
    Bluetooth4 = 4,
    Bluetooth5 = 5,
    Usb = 6,
    Unknown = 255,
}

impl Target {
    pub const fn is_bluetooth(self) -> bool {
        matches!(
            self,
            Self::Bluetooth1
                | Self::Bluetooth2
                | Self::Bluetooth3
                | Self::Bluetooth4
                | Self::Bluetooth5
        )
    }

    pub const fn from_byte(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Prospector),
            1 => Some(Self::Bluetooth1),
            2 => Some(Self::Bluetooth2),
            3 => Some(Self::Bluetooth3),
            4 => Some(Self::Bluetooth4),
            5 => Some(Self::Bluetooth5),
            6 => Some(Self::Usb),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum TargetConnection {
    #[default]
    Unknown = 0,
    Offline = 1,
    Connecting = 2,
    Connected = 3,
}

impl TargetConnection {
    pub const fn from_byte(value: u8) -> Self {
        match value {
            1 => Self::Offline,
            2 => Self::Connecting,
            3 => Self::Connected,
            _ => Self::Unknown,
        }
    }
}

pub const TOUCAN_CONFIG_LEN: usize = ENCODED_LEN + 1;

pub fn decode_toucan_config(bytes: &[u8]) -> (Target, Settings) {
    let settings = bytes
        .get(..ENCODED_LEN)
        .and_then(|data| Settings::decode(data).ok())
        .unwrap_or_default();
    let target = match bytes {
        [.., selected] if bytes.len() == TOUCAN_CONFIG_LEN => Target::from_byte(*selected),
        _ => None,
    }
    .unwrap_or(if settings.direct_bluetooth {
        Target::Bluetooth1
    } else {
        Target::Prospector
    });
    (target, settings)
}

pub fn encode_toucan_config(
    target: Target,
    settings: &Settings,
    output: &mut [u8; TOUCAN_CONFIG_LEN],
) -> Result<(), Error> {
    let mut settings = *settings;
    settings.direct_bluetooth = target.is_bluetooth();
    settings.encode(output)?;
    output[ENCODED_LEN] = target as u8;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preferences::BrailleMode;

    #[test]
    fn legacy_toucan_routes_keep_their_preferences() {
        for (direct_bluetooth, target) in [(true, Target::Bluetooth1), (false, Target::Prospector)]
        {
            let mut settings = Settings {
                mode: BrailleMode::Ueb1,
                direct_bluetooth,
                ..Settings::default()
            };
            settings.set_binding(2, &[1], b"tea").unwrap();
            let mut original = [0; ENCODED_LEN];
            settings.encode(&mut original).unwrap();
            assert_eq!(decode_toucan_config(&original), (target, settings));
        }
        assert_eq!(
            decode_toucan_config(&[]),
            (Target::Prospector, Settings::default())
        );
    }

    #[test]
    fn each_target_survives_toucan_config_round_trip() {
        let mut bytes = [0; TOUCAN_CONFIG_LEN];
        let mut settings = Settings::default();
        settings.set_binding(3, &[1, 3], b"tea").unwrap();
        for code in 0..=6 {
            let target = Target::from_byte(code).unwrap();
            encode_toucan_config(target, &settings, &mut bytes).unwrap();
            let (actual_target, actual_settings) = decode_toucan_config(&bytes);
            assert_eq!(actual_target, target);
            assert_eq!(actual_settings.direct_bluetooth, (1..=5).contains(&code));
            assert_eq!(actual_settings.bindings, settings.bindings);
        }
    }
}
