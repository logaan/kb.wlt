use crate::split::sequence_is_newer;

pub const PACKET_LEN: usize = 24;
pub const DISPLAY_PACKET_LEN: usize = 7 + 48 + 48;
pub const DISPLAY_TIMEOUT_MS: u32 = 600;
pub const LINK_TIMEOUT_MS: u32 = 500;
pub const KEYS_MASK: u32 = (1 << 21) - 1;
pub const BRAILLE_THUMB: u8 = 36;
pub const BRAILLE_THUMBS: u64 = (1 << BRAILLE_THUMB) | (1 << 41);
const BRAILLE_HOLD_MS: u32 = 200;

#[derive(Default)]
pub struct BrailleSwitch {
    latched: bool,
    pressed_at: Option<u32>,
    held: bool,
}

impl BrailleSwitch {
    pub fn update(&mut self, now: u32, keys: Option<u64>) -> bool {
        match keys {
            Some(keys) if keys & BRAILLE_THUMBS != 0 => {
                let start = *self.pressed_at.get_or_insert(now);
                self.held |=
                    now.wrapping_sub(start) >= BRAILLE_HOLD_MS || keys & !BRAILLE_THUMBS != 0;
            }
            Some(_) => {
                if let Some(start) = self.pressed_at.take() {
                    if !self.held && now.wrapping_sub(start) < BRAILLE_HOLD_MS {
                        self.latched = !self.latched;
                    }
                }
                self.held = false;
            }
            None => {
                self.pressed_at = None;
                self.held = false;
            }
        }
        self.latched || self.held
    }
}

pub fn braille_transition_blocked(entering: bool, previous: u64, keys: u64) -> u64 {
    if entering {
        previous & keys & !BRAILLE_THUMBS
    } else {
        keys
    }
}

pub fn joined_matrix(left: u32, right: u64) -> u64 {
    Side::Left.map(left) | right
}

pub fn braille_keys(matrix: u64) -> u16 {
    [
        (16, 31),
        (15, 32),
        (14, 33),
        (28, 19),
        (27, 20),
        (26, 21),
        (13, 34),
        (25, 22),
        (38, 39),
    ]
    .into_iter()
    .enumerate()
    .fold(0, |cell, (dot, (left, right))| {
        cell | (u16::from(matrix & ((1 << left) | (1 << right)) != 0) << dot)
    })
}

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
    pub battery: u8,
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
        bytes[20] = crate::battery::encode(self.battery);
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
        if keys & !KEYS_MASK != 0 || bytes[21] != 0 {
            return Err(PacketError::Keys);
        }
        if checksum(&bytes[..22]) != u16::from_le_bytes([bytes[22], bytes[23]]) {
            return Err(PacketError::Checksum);
        }
        Ok(Self {
            battery: crate::battery::decode(bytes[20]).ok_or(PacketError::Status)?,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayFeedback {
    pub sequence: u16,
    pub model: crate::pipeline::Bounded<u8, 48>,
    pub detail: crate::pipeline::Bounded<u8, 48>,
}

impl DisplayFeedback {
    pub fn encode(self) -> crate::pipeline::Bounded<u8, DISPLAY_PACKET_LEN> {
        let [low, high] = self.sequence.to_le_bytes();
        let mut bytes = crate::pipeline::Bounded::from_slice(
            &[
                2,
                low,
                high,
                self.model.len() as u8,
                self.detail.len() as u8,
            ],
            0,
        )
        .unwrap();
        bytes.extend_from_slice(self.model.as_slice()).unwrap();
        bytes.extend_from_slice(self.detail.as_slice()).unwrap();
        bytes
            .extend_from_slice(&checksum(bytes.as_slice()).to_le_bytes())
            .unwrap();
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, PacketError> {
        if bytes.len() < 7 || bytes.len() > DISPLAY_PACKET_LEN {
            return Err(PacketError::Length);
        }
        if bytes[0] != 2 {
            return Err(PacketError::Version);
        }
        let m = usize::from(bytes[3]);
        let d = usize::from(bytes[4]);
        if m > 48 || d > 48 || bytes.len() != 7 + m + d {
            return Err(PacketError::Length);
        }
        let (body, crc) = bytes.split_at(5 + m + d);
        if checksum(body).to_le_bytes() != crc {
            return Err(PacketError::Checksum);
        }
        let detail = &body[5 + m..];
        core::str::from_utf8(detail).map_err(|_| PacketError::Status)?;
        Ok(Self {
            sequence: u16::from_le_bytes([bytes[1], bytes[2]]),
            model: crate::pipeline::Bounded::from_slice(&bytes[5..5 + m], 0).unwrap(),
            detail: crate::pipeline::Bounded::from_slice(detail, 0).unwrap(),
        })
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
    pub fn battery(&self) -> u8 {
        self.snapshot.map_or(255, |s| s.battery)
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
