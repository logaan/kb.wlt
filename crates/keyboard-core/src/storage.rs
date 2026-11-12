pub const HEADER_LEN: usize = 16;
pub const MAX_PAYLOAD_LEN: usize = 512;
pub const MAX_ENCODED_LEN: usize = HEADER_LEN + MAX_PAYLOAD_LEN;
pub const COMMIT_LEN: usize = 4;
pub const MAGIC: [u8; 4] = *b"KBND";
pub const VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageError {
    BufferTooSmall,
    PayloadTooLarge,
    MissingCommit,
    UnsupportedVersion,
    Checksum,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Envelope<'a> {
    pub generation: u32,
    pub payload: &'a [u8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectedEnvelope<'a> {
    pub slot: usize,
    pub envelope: Envelope<'a>,
}

pub fn encode(
    payload: &[u8],
    generation: u32,
    destination: &mut [u8],
) -> Result<usize, StorageError> {
    if payload.len() > MAX_PAYLOAD_LEN {
        return Err(StorageError::PayloadTooLarge);
    }
    let encoded_len = (HEADER_LEN + payload.len() + 3) & !3;
    if destination.len() < encoded_len {
        return Err(StorageError::BufferTooSmall);
    }
    let output = &mut destination[..encoded_len];
    output.fill(0xff);
    output[..4].copy_from_slice(&MAGIC);
    output[4..8].copy_from_slice(&generation.to_le_bytes());
    output[8..10].copy_from_slice(&(payload.len() as u16).to_le_bytes());
    output[10..12].copy_from_slice(&VERSION.to_le_bytes());
    output[HEADER_LEN..HEADER_LEN + payload.len()].copy_from_slice(payload);
    let checksum = crc32(&[&output[4..12], payload]);
    output[12..16].copy_from_slice(&checksum.to_le_bytes());
    Ok(encoded_len)
}

pub fn decode(source: &[u8]) -> Result<Envelope<'_>, StorageError> {
    if source.len() < HEADER_LEN {
        return Err(StorageError::BufferTooSmall);
    }
    if source[..4] != MAGIC {
        return Err(StorageError::MissingCommit);
    }
    let generation = u32::from_le_bytes(source[4..8].try_into().unwrap());
    let payload_len = u16::from_le_bytes(source[8..10].try_into().unwrap()) as usize;
    let version = u16::from_le_bytes(source[10..12].try_into().unwrap());
    if version != VERSION {
        return Err(StorageError::UnsupportedVersion);
    }
    if payload_len > MAX_PAYLOAD_LEN {
        return Err(StorageError::PayloadTooLarge);
    }
    let payload = source
        .get(HEADER_LEN..HEADER_LEN + payload_len)
        .ok_or(StorageError::BufferTooSmall)?;
    let expected = u32::from_le_bytes(source[12..16].try_into().unwrap());
    if crc32(&[&source[4..12], payload]) != expected {
        return Err(StorageError::Checksum);
    }
    Ok(Envelope {
        generation,
        payload,
    })
}

pub fn select_newest<'a>(first: &'a [u8], second: &'a [u8]) -> Option<SelectedEnvelope<'a>> {
    match (decode(first), decode(second)) {
        (Err(_), Err(_)) => None,
        (Ok(envelope), Err(_)) => Some(SelectedEnvelope { slot: 0, envelope }),
        (Err(_), Ok(envelope)) => Some(SelectedEnvelope { slot: 1, envelope }),
        (Ok(first), Ok(second)) => {
            if generation_is_newer(second.generation, first.generation) {
                Some(SelectedEnvelope {
                    slot: 1,
                    envelope: second,
                })
            } else {
                Some(SelectedEnvelope {
                    slot: 0,
                    envelope: first,
                })
            }
        }
    }
}

pub const fn generation_is_newer(candidate: u32, previous: u32) -> bool {
    (candidate.wrapping_sub(previous) as i32) > 0
}

fn crc32(parts: &[&[u8]]) -> u32 {
    let mut crc = u32::MAX;
    for part in parts {
        for byte in *part {
            crc ^= *byte as u32;
            for _ in 0..8 {
                let low = crc & 1;
                crc = (crc >> 1) ^ (0xedb8_8320_u32.wrapping_mul(low));
            }
        }
    }
    !crc
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SaveState {
    revision: u32,
    persisted: u32,
    failed: bool,
}

impl SaveState {
    pub const fn new() -> Self {
        Self {
            revision: 0,
            persisted: 0,
            failed: false,
        }
    }

    pub fn requested(self) -> (Self, u32) {
        let revision = self.revision.wrapping_add(1);
        (Self { revision, ..self }, revision)
    }

    pub fn completed(self, revision: u32, success: bool) -> Self {
        Self {
            persisted: if success { revision } else { self.persisted },
            failed: !success,
            ..self
        }
    }

    pub const fn unsaved(self) -> bool {
        self.failed || self.revision != self.persisted
    }
}
