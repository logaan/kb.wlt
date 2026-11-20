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
