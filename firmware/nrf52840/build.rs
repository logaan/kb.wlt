use std::{env, fs, path::PathBuf};

#[path = "../../clients/build/embedded/mod.rs"]
mod embedded;

fn main() {
    if env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("arm") {
        return;
    }
    let origin = env::var("KB_FLASH_ORIGIN").unwrap_or_else(|_| "0x26000".into());
    let origin = u32::from_str_radix(origin.trim_start_matches("0x"), 16)
        .expect("KB_FLASH_ORIGIN must be a hexadecimal address");
    assert!(matches!(origin, 0x1000 | 0x26000 | 0x27000));
    let end = 0xe9000;
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let keyboard = env::var_os("CARGO_FEATURE_TOUCAN_DONGLE").is_some()
        || env::var_os("CARGO_FEATURE_TOUCAN_DISPLAY").is_some();
    if keyboard || env::var_os("CARGO_FEATURE_PROSPECTOR_DISPLAY").is_some() {
        let (key, file) = if keyboard {
            ("KB_KEYBOARD_WASM", "toucan-keymap/keyboard.core.wasm")
        } else {
            ("KB_BRAILLE_WASM", "braille.core.wasm")
        };
        let path = env::var_os(key)
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("../../clients/components/generated").join(file));
        let path = fs::canonicalize(path).expect("WebAssembly component artifact is missing");
        embedded::compile(
            &path,
            &out,
            &root.join("../../host/runtime/embedded"),
            keyboard,
        );
        println!("cargo:rerun-if-env-changed={key}");
    }
    let mut memory = format!(
        "MEMORY {{\n  FLASH : ORIGIN = {origin:#x}, LENGTH = {length:#x}\n  RAM : ORIGIN = 0x20000008, LENGTH = 0x3fff0\n}}\n",
        length = end - origin
    );
    if keyboard {
        memory.push_str("ASSERT(_stack_start >= _stack_end + 26K, \"Toucan runtime requires at least 26 KiB of native stack\")\n");
    }
    fs::write(out.join("memory.x"), memory).unwrap();
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rustc-link-arg=-Tlink.x");
    println!("cargo:rerun-if-env-changed=KB_FLASH_ORIGIN");
    println!("cargo:rerun-if-changed=build.rs");
}
