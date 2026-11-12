use std::collections::BTreeMap;
use std::fmt;

pub mod targets;

pub const NRF52840_FAMILY: u32 = 0xada5_2840;
pub const FLASH_END: u32 = 0x000f_0000;
pub const STOCK_APPLICATION_START: u32 = 0x0002_6000;
pub const S140_V7_APPLICATION_START: u32 = 0x0002_7000;
pub const NOSD_APPLICATION_START: u32 = 0x0000_1000;
const BLOCK_SIZE: usize = 512;
const PAYLOAD_SIZE: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageError(pub String);

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for ImageError {}

type Result<T> = std::result::Result<T, ImageError>;

fn reject<T>(message: impl Into<String>) -> Result<T> {
    Err(ImageError(message.into()))
}

fn range(bytes: &[u8], start: usize, len: usize) -> Result<&[u8]> {
    let end = start
        .checked_add(len)
        .ok_or_else(|| ImageError("ELF range overflow".into()))?;
    bytes
        .get(start..end)
        .ok_or_else(|| ImageError("truncated ELF".into()))
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        range(bytes, offset, 2)?.try_into().unwrap(),
    ))
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        range(bytes, offset, 4)?.try_into().unwrap(),
    ))
}

fn header_only_segment(bytes: &[u8], file_size: usize) -> Result<bool> {
    let section_offset = u32_at(bytes, 32)? as usize;
    let section_size = u16_at(bytes, 46)? as usize;
    let section_count = u16_at(bytes, 48)? as usize;
    if section_size != 40 || section_count == 0 {
        return Ok(false);
    }
    for index in 0..section_count {
        let section = range(bytes, section_offset + index * section_size, section_size)?;
        let kind = u32_at(section, 4)?;
        let flags = u32_at(section, 8)?;
        let offset = u32_at(section, 16)? as usize;
        let size = u32_at(section, 20)?;
        if kind != 8 && flags & 2 != 0 && size != 0 && offset < file_size {
            return Ok(false);
        }
    }
    Ok(true)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirmwareImage {
    pub base_address: u32,
    pub data: Vec<u8>,
}

impl FirmwareImage {
    pub fn from_elf(bytes: &[u8], application_start: u32) -> Result<Self> {
        if !matches!(
            application_start,
            STOCK_APPLICATION_START | S140_V7_APPLICATION_START | NOSD_APPLICATION_START
        ) {
            return reject(
                "application address must be 0x26000 (S140 v6), 0x27000 (S140 v7) or 0x1000 (nosd)",
            );
        }
        if range(bytes, 0, 7)? != b"\x7fELF\x01\x01\x01" {
            return reject("expected ELF32 little-endian firmware");
        }
        if u16_at(bytes, 16)? != 2 || u16_at(bytes, 18)? != 40 {
            return reject("expected an ARM executable");
        }
        let entry = u32_at(bytes, 24)?;
        let headers = u32_at(bytes, 28)? as usize;
        let header_size = u16_at(bytes, 42)? as usize;
        let header_count = u16_at(bytes, 44)? as usize;
        if header_size != 32 || header_count == 0 {
            return reject("invalid ELF program headers");
        }
        let mut segments = BTreeMap::new();
        let mut end = application_start;
        for i in 0..header_count {
            let header = range(bytes, headers + i * header_size, header_size)?;
            if u32_at(header, 0)? != 1 {
                continue;
            }
            let file_size = u32_at(header, 16)?;
            if file_size == 0 {
                continue;
            }
            let file_offset = u32_at(header, 4)? as usize;
            if file_offset == 0
                && u32_at(header, 24)? == 4
                && header_only_segment(bytes, file_size as usize)?
            {
                continue;
            }
            let address = u32_at(header, 12)?;
            let segment_end = address
                .checked_add(file_size)
                .ok_or_else(|| ImageError("ELF address overflow".into()))?;
            if address < application_start || segment_end > FLASH_END {
                return reject(format!(
                    "load segment {address:#010x}..{segment_end:#010x} lies outside application flash"
                ));
            }
            if file_size > u32_at(header, 20)? {
                return reject("ELF file size exceeds segment memory size");
            }
            let data = range(bytes, file_offset, file_size as usize)?;
            if segments.insert(address, data).is_some() {
                return reject("duplicate ELF load address");
            }
            end = end.max(segment_end);
        }
        if segments.first_key_value().map(|(address, _)| *address) != Some(application_start) {
            return reject(
                "firmware vector table does not start at the selected application address",
            );
        }
        let mut data = vec![0xff; (end - application_start) as usize];
        let mut previous_end = application_start;
        for (address, segment) in segments {
            if address < previous_end {
                return reject("overlapping ELF load segments");
            }
            let offset = (address - application_start) as usize;
            data[offset..offset + segment.len()].copy_from_slice(segment);
            previous_end = address + segment.len() as u32;
        }
        let stack = u32_at(&data, 0)?;
        let reset = u32_at(&data, 4)?;
        if !(0x2000_0008..=0x2004_0000).contains(&stack) || stack % 8 != 0 {
            return reject("invalid nRF52840 initial stack pointer");
        }
        if reset & 1 == 0 || reset < application_start || reset >= end || reset != entry {
            return reject("invalid Cortex-M reset vector or ELF entry point");
        }
        Ok(Self {
            base_address: application_start,
            data,
        })
    }

    pub fn uf2(&self) -> Result<Vec<u8>> {
        if self.base_address % PAYLOAD_SIZE as u32 != 0 || self.data.is_empty() {
            return reject("UF2 requires a nonempty, 256-byte aligned image");
        }
        let count = self.data.len().div_ceil(PAYLOAD_SIZE);
        let end = u64::from(self.base_address) + (count * PAYLOAD_SIZE) as u64;
        if self.base_address < NOSD_APPLICATION_START || end > u64::from(FLASH_END) {
            return reject("UF2 image would overwrite reserved flash");
        }
        let mut output = vec![0; count * BLOCK_SIZE];
        for (index, payload) in self.data.chunks(PAYLOAD_SIZE).enumerate() {
            let block = &mut output[index * BLOCK_SIZE..(index + 1) * BLOCK_SIZE];
            for (offset, value) in [
                (0, 0x0a32_4655u32),
                (4, 0x9e5d_5157),
                (8, 0x0000_2000),
                (12, self.base_address + (index * PAYLOAD_SIZE) as u32),
                (16, PAYLOAD_SIZE as u32),
                (20, index as u32),
                (24, count as u32),
                (28, NRF52840_FAMILY),
                (508, 0x0ab1_6f30),
            ] {
                block[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            }
            block[32..32 + PAYLOAD_SIZE].fill(0xff);
            block[32..32 + payload.len()].copy_from_slice(payload);
        }
        Ok(output)
    }
}
