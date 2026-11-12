use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=KB_FLASH_ORIGIN");
    println!("cargo:rerun-if-changed=build.rs");
    if env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("arm") {
        return;
    }
    let origin = env::var("KB_FLASH_ORIGIN").unwrap_or_else(|_| "0x27000".into());
    let origin = u32::from_str_radix(origin.trim_start_matches("0x"), 16)
        .expect("KB_FLASH_ORIGIN must be a hexadecimal address");
    assert!(matches!(origin, 0x1000 | 0x26000 | 0x27000));
    let memory = format!(
        "MEMORY {{\n  FLASH : ORIGIN = {origin:#x}, LENGTH = {length:#x}\n  RAM : ORIGIN = 0x20000008, LENGTH = 0x3fff0\n}}\n",
        length = 0xf4000 - origin,
    );
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    fs::write(out.join("memory.x"), memory).unwrap();
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rustc-link-arg=-Tlink.x");
}
