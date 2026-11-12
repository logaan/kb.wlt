use crate::split::sequence_is_newer;

pub const PACKET_LEN: usize = 24;
pub const DISPLAY_PACKET_LEN: usize = 10;
pub const DISPLAY_TIMEOUT_MS: u32 = 600;
pub const LINK_TIMEOUT_MS: u32 = 500;
pub const KEYS_MASK: u32 = (1 << 21) - 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Side {
    Left = 1,
    Right = 2,
}

impl Side {
    pub const fn index(self) -> usize {
        self as usize - 1
    }

    pub const fn position(self, local: u8) -> Option<u8> {
        if local >= 21 {
            return None;
        }
        Some(if local < 18 {
            local / 6 * 12
                + local % 6
                + match self {
                    Self::Left => 0,
                    Self::Right => 6,
                }
        } else {
            36 + local - 18
                + match self {
                    Self::Left => 0,
                    Self::Right => 3,
                }
        })
    }

    pub fn map(self, keys: u32) -> u64 {
        (0..21).fold(0, |mapped, index| {
            if keys & (1 << index) != 0 {
                mapped | (1u64 << self.position(index).unwrap())
            } else {
                mapped
            }
        })
    }

    pub const fn matrix_index(self, row: u8, column: u8) -> Option<u8> {
        if row > 3 || column > 5 {
            return None;
        }
        if row < 3 {
            return Some(row * 6 + column);
        }
        match self {
            Self::Left if column >= 3 => Some(18 + column - 3),
            Self::Right if column < 3 => Some(18 + column),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub node: u64,
    pub boot: u32,
    pub sequence: u16,
    pub side: Side,
    pub keys: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PacketError {
    Length,
    Version,
    Side,
    Keys,
    Checksum,
    Status,
}

impl Snapshot {
    pub fn encode(self) -> [u8; PACKET_LEN] {
        let mut bytes = [0; PACKET_LEN];
        bytes[0] = 2;
        bytes[1] = self.side as u8;
        bytes[2..10].copy_from_slice(&self.node.to_le_bytes());
        bytes[10..14].copy_from_slice(&self.boot.to_le_bytes());
        bytes[14..16].copy_from_slice(&self.sequence.to_le_bytes());
        bytes[16..20].copy_from_slice(&self.keys.to_le_bytes());
        let crc = checksum(&bytes[..22]);
        bytes[22..24].copy_from_slice(&crc.to_le_bytes());
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, PacketError> {
        if bytes.len() != PACKET_LEN {
            return Err(PacketError::Length);
        }
        if bytes[0] != 2 {
            return Err(PacketError::Version);
        }
        let side = match bytes[1] {
            1 => Side::Left,
            2 => Side::Right,
            _ => return Err(PacketError::Side),
        };
        let keys = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
        if keys & !KEYS_MASK != 0 || bytes[20] != 0 || bytes[21] != 0 {
            return Err(PacketError::Keys);
        }
        if checksum(&bytes[..22]) != u16::from_le_bytes([bytes[22], bytes[23]]) {
            return Err(PacketError::Checksum);
        }
        Ok(Self {
            node: u64::from_le_bytes(bytes[2..10].try_into().unwrap()),
            boot: u32::from_le_bytes(bytes[10..14].try_into().unwrap()),
            sequence: u16::from_le_bytes([bytes[14], bytes[15]]),
            side,
            keys,
        })
    }
}

fn checksum(bytes: &[u8]) -> u16 {
    let mut crc = 0xffff;
    for byte in bytes {
        crc ^= (*byte as u16) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DisplayStatus {
    pub connected: bool,
    pub faulted: bool,
    pub pairing_unsaved: bool,
    pub active_keys: u8,
    pub layer: u8,
    pub last_usage: u8,
    pub last_modifiers: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayFeedback {
    pub sequence: u16,
    pub status: DisplayStatus,
}

impl DisplayFeedback {
    pub fn encode(self) -> [u8; DISPLAY_PACKET_LEN] {
        let status = self.status;
        let mut bytes = [0; DISPLAY_PACKET_LEN];
        bytes[0] = 1;
        bytes[1] = u8::from(status.connected)
            | u8::from(status.faulted) << 1
            | u8::from(status.pairing_unsaved) << 2;
        bytes[2..4].copy_from_slice(&self.sequence.to_le_bytes());
        bytes[4..8].copy_from_slice(&[
            status.active_keys,
            status.layer,
            status.last_usage,
            status.last_modifiers,
        ]);
        let crc = checksum(&bytes[..8]);
        bytes[8..].copy_from_slice(&crc.to_le_bytes());
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, PacketError> {
        if bytes.len() != DISPLAY_PACKET_LEN {
            return Err(PacketError::Length);
        }
        if bytes[0] != 1 {
            return Err(PacketError::Version);
        }
        if checksum(&bytes[..8]) != u16::from_le_bytes([bytes[8], bytes[9]]) {
            return Err(PacketError::Checksum);
        }
        let flags = bytes[1];
        if flags & !0b111 != 0 || bytes[4] > 42 {
            return Err(PacketError::Status);
        }
        Ok(Self {
            sequence: u16::from_le_bytes([bytes[2], bytes[3]]),
            status: DisplayStatus {
                connected: flags & 1 != 0,
                faulted: flags & 2 != 0,
                pairing_unsaved: flags & 4 != 0,
                active_keys: bytes[4],
                layer: bytes[5],
                last_usage: bytes[6],
                last_modifiers: bytes[7],
            },
        })
    }
}

#[derive(Default)]
pub struct DisplayReceiver {
    last: Option<(DisplayFeedback, u32)>,
}

impl DisplayReceiver {
    pub fn receive(&mut self, feedback: DisplayFeedback, now_ms: u32) {
        if self
            .last
            .is_none_or(|(previous, _)| sequence_is_newer(feedback.sequence, previous.sequence))
        {
            self.last = Some((feedback, now_ms));
        }
    }

    pub fn current(&self, now_ms: u32) -> Option<DisplayStatus> {
        let (feedback, received_at) = self.last?;
        (now_ms.wrapping_sub(received_at) < DISPLAY_TIMEOUT_MS).then_some(feedback.status)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Receiver {
    snapshot: Option<Snapshot>,
    last_seen: u32,
    armed: bool,
    keys: u64,
}

impl Default for Receiver {
    fn default() -> Self {
        Self::new()
    }
}

impl Receiver {
    pub const fn new() -> Self {
        Self {
            snapshot: None,
            last_seen: 0,
            armed: false,
            keys: 0,
        }
    }
    pub const fn keys(&self) -> u64 {
        self.keys
    }
    pub const fn connected(&self) -> bool {
        self.snapshot.is_some()
    }
    pub fn close(&mut self) {
        *self = Self::new();
    }
    pub fn expire(&mut self, now: u32) -> bool {
        if self.connected() && now.wrapping_sub(self.last_seen) >= LINK_TIMEOUT_MS {
            self.close();
            true
        } else {
            false
        }
    }
    pub fn receive_queued(
        &mut self,
        snapshot: Snapshot,
        received_at: u32,
        now: u32,
        opens: bool,
    ) -> Result<bool, PacketError> {
        if now.wrapping_sub(received_at) >= LINK_TIMEOUT_MS {
            return Ok(false);
        }
        let replaced = opens && self.connected();
        let mut next = if replaced { Self::new() } else { *self };
        let reset = next.receive(snapshot, received_at)?;
        *self = next;
        Ok(replaced || reset)
    }

    pub fn receive(&mut self, snapshot: Snapshot, now: u32) -> Result<bool, PacketError> {
        if snapshot.keys & !KEYS_MASK != 0 {
            return Err(PacketError::Keys);
        }
        let mut reset = false;
        if let Some(previous) = self.snapshot {
            if previous.node != snapshot.node || previous.side != snapshot.side {
                return Err(PacketError::Side);
            }
            if previous.boot == snapshot.boot {
                if !sequence_is_newer(snapshot.sequence, previous.sequence) {
                    return Ok(false);
                }
            } else {
                self.armed = false;
                self.keys = 0;
                reset = true;
            }
        }
        self.snapshot = Some(snapshot);
        self.last_seen = now;
        if snapshot.keys == 0 {
            self.armed = true;
        }
        self.keys = if self.armed {
            snapshot.side.map(snapshot.keys)
        } else {
            0
        };
        Ok(reset)
    }
}
