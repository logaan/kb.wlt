use xtask::{FirmwareImage, NRF52840_FAMILY, STOCK_APPLICATION_START as BASE};

fn put16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn read32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn elf() -> Vec<u8> {
    let mut bytes = vec![0; 0x280];
    bytes[..7].copy_from_slice(b"\x7fELF\x01\x01\x01");
    put16(&mut bytes, 16, 2);
    put16(&mut bytes, 18, 40);
    put32(&mut bytes, 24, BASE + 9);
    put32(&mut bytes, 28, 52);
    put16(&mut bytes, 42, 32);
    put16(&mut bytes, 44, 2);
    for (header, offset, address, length) in
        [(52, 0x100, BASE, 0x100), (84, 0x200, BASE + 0x180, 0x80)]
    {
        put32(&mut bytes, header, 1);
        put32(&mut bytes, header + 4, offset);
        put32(
            &mut bytes,
            header + 8,
            if header == 84 { 0x2000_0000 } else { address },
        );
        put32(&mut bytes, header + 12, address);
        put32(&mut bytes, header + 16, length);
        put32(&mut bytes, header + 20, length);
    }
    put32(&mut bytes, 0x100, 0x2004_0000);
    put32(&mut bytes, 0x104, BASE + 9);
    bytes[0x200..].fill(0x55);
    bytes
}

#[test]
fn converts_physical_load_addresses_including_initialized_ram() {
    let image = FirmwareImage::from_elf(&elf(), BASE).unwrap();
    assert_eq!(image.base_address, BASE);
    assert_eq!(image.data.len(), 512);
    assert!(image.data[0x100..0x180].iter().all(|b| *b == 0xff));
    assert!(image.data[0x180..].iter().all(|b| *b == 0x55));
    let uf2 = image.uf2().unwrap();
    assert_eq!(uf2.len(), 1024);
    for (index, block) in uf2.chunks(512).enumerate() {
        assert_eq!(read32(block, 0), 0x0a32_4655);
        assert_eq!(read32(block, 4), 0x9e5d_5157);
        assert_eq!(read32(block, 8), 0x2000);
        assert_eq!(read32(block, 12), BASE + index as u32 * 256);
        assert_eq!(read32(block, 16), 256);
        assert_eq!(read32(block, 20), index as u32);
        assert_eq!(read32(block, 24), 2);
        assert_eq!(read32(block, 28), NRF52840_FAMILY);
        assert_eq!(read32(block, 508), 0x0ab1_6f30);
        assert_eq!(&block[32..288], &image.data[index * 256..(index + 1) * 256]);
    }
}

#[test]
fn rejects_unsafe_or_corrupt_firmware() {
    for (offset, value) in [
        (52 + 12, 0),
        (84 + 12, 0x000f_4000),
        (84 + 12, BASE + 0x80),
        (84 + 12, u32::MAX),
        (84 + 16, 0x1000),
        (0x100, 0x2004_0008),
        (0x104, BASE + 8),
        (24, BASE + 11),
    ] {
        let mut bytes = elf();
        put32(&mut bytes, offset, value);
        assert!(
            FirmwareImage::from_elf(&bytes, BASE).is_err(),
            "offset {offset}, value {value:x}"
        );
    }
    let bytes = elf();
    for len in 0..bytes.len() {
        assert!(FirmwareImage::from_elf(&bytes[..len], BASE).is_err());
    }
}

#[test]
fn padding_does_not_cross_bootloader_boundary() {
    let image = FirmwareImage {
        base_address: 0xf3f00,
        data: vec![0; 257],
    };
    assert!(image.uf2().is_err());
    let image = FirmwareImage {
        base_address: BASE,
        data: vec![0x5a; 257],
    };
    let uf2 = image.uf2().unwrap();
    assert_eq!(uf2[512 + 32], 0x5a);
    assert!(uf2[512 + 33..512 + 288].iter().all(|b| *b == 0xff));
}

#[test]
fn skips_linker_generated_read_only_elf_header_segment() {
    let mut bytes = elf();
    bytes.resize(0x280 + 80, 0);
    put32(&mut bytes, 32, 0x280);
    put16(&mut bytes, 46, 40);
    put16(&mut bytes, 48, 2);
    put32(&mut bytes, 0x280 + 40 + 4, 1);
    put32(&mut bytes, 0x280 + 40 + 8, 2);
    put32(&mut bytes, 0x280 + 40 + 16, 0x100);
    put32(&mut bytes, 0x280 + 40 + 20, 0x180);
    put16(&mut bytes, 44, 3);
    put32(&mut bytes, 116, 1);
    put32(&mut bytes, 120, 0);
    put32(&mut bytes, 124, 0x20000);
    put32(&mut bytes, 128, 0x20000);
    put32(&mut bytes, 132, 148);
    put32(&mut bytes, 136, 148);
    put32(&mut bytes, 140, 4);
    assert!(FirmwareImage::from_elf(&bytes, BASE).is_ok());
    put32(&mut bytes, 140, 5);
    assert!(FirmwareImage::from_elf(&bytes, BASE).is_err());
    put32(&mut bytes, 140, 4);
    put32(&mut bytes, 132, 257);
    assert!(FirmwareImage::from_elf(&bytes, BASE).is_err());
}
