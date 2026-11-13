pub const PACKET_LEN: usize = 20;
pub const PROTOCOL_VERSION: u8 = 1;
pub const SINGLE_PAINTBRUSH: [u16; 8] = [0x40, 0x04, 0x02, 0x01, 0x08, 0x10, 0x20, 0x100];
pub const LEFT_PAINTBRUSH: [u16; 8] = [0x40, 0x04, 0x02, 0x01, 0, 0, 0, 0x100];
pub const RIGHT_PAINTBRUSH: [u16; 8] = [0x08, 0x10, 0x20, 0x80, 0x100, 0, 0, 0];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum Layout {
    #[default]
    Single = 0,
    Left = 1,
    Right = 2,
}

impl Layout {
    pub const fn map(self, keys: u8) -> u16 {
        map_keys(
            keys,
            match self {
                Self::Single => &SINGLE_PAINTBRUSH,
                Self::Left => &LEFT_PAINTBRUSH,
                Self::Right => &RIGHT_PAINTBRUSH,
            },
        )
    }
}

pub const fn map_keys(keys: u8, mapping: &[u16; 8]) -> u16 {
    let mut mapped = 0;
    let mut index = 0;
    while index < mapping.len() {
        if keys & (1 << index) != 0 {
            mapped |= mapping[index];
        }
        index += 1;
    }
    mapped
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub node: u64,
    pub boot: u32,
    pub sequence: u16,
    pub layout: Layout,
    pub keys: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PacketError {
    Length,
    Version,
    Layout,
    Keys,
    Checksum,
}

impl Snapshot {
    pub fn encode(self) -> [u8; PACKET_LEN] {
        let mut bytes = [0; PACKET_LEN];
        bytes[0] = PROTOCOL_VERSION;
        bytes[1] = self.layout as u8;
        bytes[2..10].copy_from_slice(&self.node.to_le_bytes());
        bytes[10..14].copy_from_slice(&self.boot.to_le_bytes());
        bytes[14..16].copy_from_slice(&self.sequence.to_le_bytes());
        bytes[16] = self.keys;
        let checksum = crc16(&bytes[..18]);
        bytes[18..20].copy_from_slice(&checksum.to_le_bytes());
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, PacketError> {
        if bytes.len() != PACKET_LEN {
            return Err(PacketError::Length);
        }
        if bytes[0] != PROTOCOL_VERSION {
            return Err(PacketError::Version);
        }
        let layout = match bytes[1] {
            0 => Layout::Single,
            1 => Layout::Left,
            2 => Layout::Right,
            _ => return Err(PacketError::Layout),
        };
        if bytes[17] != 0 {
            return Err(PacketError::Keys);
        }
        if crc16(&bytes[..18]) != u16::from_le_bytes([bytes[18], bytes[19]]) {
            return Err(PacketError::Checksum);
        }
        Ok(Self {
            node: u64::from_le_bytes(bytes[2..10].try_into().unwrap()),
            boot: u32::from_le_bytes(bytes[10..14].try_into().unwrap()),
            sequence: u16::from_le_bytes(bytes[14..16].try_into().unwrap()),
            layout,
            keys: bytes[16],
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Session {
    pub slot: usize,
    pub generation: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Peer {
    node: u64,
    layout: Layout,
    generation: u32,
    boot: Option<u32>,
    sequence: u16,
    last_seen_ms: u32,
    keys: u16,
    connected: bool,
    armed: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Update {
    pub keys: u16,
    pub pressed: u16,
    pub released: u16,
    pub cancel_chord: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReceiveError {
    Slot,
    StaleSession,
    Identity,
    Layout,
    BootChanged,
    Sequence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Receiver<const N: usize> {
    peers: [Peer; N],
    timeout_ms: u32,
}

impl<const N: usize> Receiver<N> {
    pub const fn new(timeout_ms: u32) -> Self {
        assert!(timeout_ms > 0 && timeout_ms < 0x8000_0000);
        Self {
            peers: [Peer {
                node: 0,
                layout: Layout::Single,
                generation: 0,
                boot: None,
                sequence: 0,
                last_seen_ms: 0,
                keys: 0,
                connected: false,
                armed: false,
            }; N],
            timeout_ms,
        }
    }

    pub fn keys(&self) -> u16 {
        self.peers.iter().fold(0, |keys, peer| keys | peer.keys)
    }

    pub fn is_connected(&self, slot: usize) -> bool {
        self.peers.get(slot).is_some_and(|peer| peer.connected)
    }

    pub fn connect(
        mut self,
        slot: usize,
        node: u64,
        layout: Layout,
        now_ms: u32,
    ) -> Result<(Self, Session, Update), ReceiveError> {
        let previous = self.keys();
        let peer = self.peers.get_mut(slot).ok_or(ReceiveError::Slot)?;
        let cancel_chord = peer.connected;
        let generation = peer.generation.wrapping_add(1);
        *peer = Peer {
            node,
            layout,
            generation,
            last_seen_ms: now_ms,
            connected: true,
            ..Peer::default()
        };
        Ok((
            self,
            Session { slot, generation },
            self.update(previous, cancel_chord),
        ))
    }

    pub fn receive(
        mut self,
        session: Session,
        snapshot: Snapshot,
        now_ms: u32,
    ) -> Result<(Self, Update), ReceiveError> {
        let previous = self.keys();
        let peer = self.session_peer(session)?;
        if snapshot.node != peer.node {
            return Err(ReceiveError::Identity);
        }
        if snapshot.layout != peer.layout {
            return Err(ReceiveError::Layout);
        }
        if let Some(boot) = peer.boot {
            if boot != snapshot.boot {
                return Err(ReceiveError::BootChanged);
            }
            if !sequence_is_newer(snapshot.sequence, peer.sequence) {
                return Err(ReceiveError::Sequence);
            }
        }
        peer.boot = Some(snapshot.boot);
        peer.sequence = snapshot.sequence;
        peer.last_seen_ms = now_ms;
        if snapshot.keys == 0 {
            peer.armed = true;
        }
        peer.keys = if peer.armed {
            snapshot.layout.map(snapshot.keys)
        } else {
            0
        };
        Ok((self, self.update(previous, false)))
    }

    pub fn disconnect(mut self, session: Session) -> Result<(Self, Update), ReceiveError> {
        let previous = self.keys();
        let peer = self.session_peer(session)?;
        peer.connected = false;
        peer.keys = 0;
        peer.armed = false;
        Ok((self, self.update(previous, true)))
    }

    pub fn expire(mut self, now_ms: u32) -> (Self, Update) {
        let previous = self.keys();
        let mut cancel_chord = false;
        for peer in &mut self.peers {
            if peer.connected && now_ms.wrapping_sub(peer.last_seen_ms) >= self.timeout_ms {
                cancel_chord = true;
                peer.connected = false;
                peer.keys = 0;
                peer.armed = false;
            }
        }
        (self, self.update(previous, cancel_chord))
    }

    fn session_peer(&mut self, session: Session) -> Result<&mut Peer, ReceiveError> {
        let peer = self.peers.get_mut(session.slot).ok_or(ReceiveError::Slot)?;
        if !peer.connected || peer.generation != session.generation {
            return Err(ReceiveError::StaleSession);
        }
        Ok(peer)
    }

    fn update(&self, previous: u16, cancel_chord: bool) -> Update {
        let keys = self.keys();
        Update {
            keys,
            pressed: keys & !previous,
            released: previous & !keys,
            cancel_chord,
        }
    }
}

pub const fn sequence_is_newer(candidate: u16, previous: u16) -> bool {
    (candidate.wrapping_sub(previous) as i16) > 0
}

fn crc16(bytes: &[u8]) -> u16 {
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
