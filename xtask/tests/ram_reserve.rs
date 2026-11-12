use xtask::ram_reserve;

fn put16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn elf() -> Vec<u8> {
    let mut bytes = vec![0; 0x300];
    bytes[..7].copy_from_slice(b"\x7fELF\x01\x01\x01");
    put16(&mut bytes, 16, 2);
    put16(&mut bytes, 18, 40);
    put32(&mut bytes, 28, 52);
    put16(&mut bytes, 42, 32);
    put16(&mut bytes, 44, 3);
    for (header, offset, virtual_address, physical_address, file_size, memory_size) in [
        (52, 0x100, 0x26000, 0x26000, 0x100, 0x100),
        (84, 0x200, 0x20000008, 0x26100, 0x80, 0x80),
        (116, 0x280, 0x20000088, 0x20000088, 0, 0x180),
    ] {
        put32(&mut bytes, header, 1);
        put32(&mut bytes, header + 4, offset);
        put32(&mut bytes, header + 8, virtual_address);
        put32(&mut bytes, header + 12, physical_address);
        put32(&mut bytes, header + 16, file_size);
        put32(&mut bytes, header + 20, memory_size);
    }
    bytes
}

#[test]
fn counts_ram_memory_including_uninitialized_segments() {
    assert_eq!(ram_reserve(&elf()).unwrap(), 0x2003fff8 - 0x20000208);
    let mut bytes = elf();
    put32(&mut bytes, 116 + 8, 0x2003bff8);
    put32(&mut bytes, 116 + 20, 0x4000);
    assert_eq!(ram_reserve(&bytes).unwrap(), 0);
}

#[test]
fn rejects_invalid_and_truncated_segments() {
    for (offset, value) in [
        (84 + 8, 0x20000000),
        (116 + 8, 0x2003fff0),
        (116 + 8, 0xfffffff0),
        (84 + 16, 0x81),
        (84 + 4, 0x2f0),
        (28, u32::MAX),
    ] {
        let mut bytes = elf();
        put32(&mut bytes, offset, value);
        assert!(
            ram_reserve(&bytes).is_err(),
            "offset {offset}, value {value:x}"
        );
    }
    let bytes = elf();
    for len in 0..0x280 {
        assert!(ram_reserve(&bytes[..len]).is_err(), "length {len}");
    }
}
