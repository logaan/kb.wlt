use std::{env, fs, path::PathBuf};

fn main() {
    if env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("arm") {
        return;
    }
    let origin = env::var("KB_FLASH_ORIGIN").unwrap_or_else(|_| "0x26000".into());
    let origin = u32::from_str_radix(origin.trim_start_matches("0x"), 16)
        .expect("KB_FLASH_ORIGIN must be a hexadecimal address");
    assert!(matches!(origin, 0x1000 | 0x26000 | 0x27000));
    let end = 0xf0000;
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let mut artifacts = vec![
        ("KB_LED_WASM", "led.core.wasm"),
        ("KB_BRAILLE_WASM", "braille.core.wasm"),
    ];
    if env::var_os("CARGO_FEATURE_COMPOSED").is_some() {
        artifacts.push(("KB_LED_WRAPPER_WASM", "led-dim/stage0/led.core.wasm"));
    }
    if env::var_os("CARGO_FEATURE_TOUCAN").is_some()
        && env::var_os("CARGO_FEATURE_KEYBOARD_COMPOSED").is_none()
    {
        artifacts.push(("KB_KEYBOARD_WASM", "toucan-keymap/keyboard.core.wasm"));
    }
    if env::var_os("CARGO_FEATURE_KEYBOARD_COMPOSED").is_some() {
        artifacts.push((
            "KB_KEYBOARD_STAGE0_WASM",
            "keyboard-composed/stage0/keyboard.core.wasm",
        ));
        artifacts.push((
            "KB_KEYBOARD_STAGE1_WASM",
            "keyboard-composed/stage1/keyboard.core.wasm",
        ));
    }
    for (key, file) in artifacts {
        let path = env::var_os(key)
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("../../clients/components/generated").join(file));
        let path = fs::canonicalize(path).expect("WebAssembly component artifact is missing");
        println!("cargo:rustc-env={key}={}", path.display());
        println!("cargo:rerun-if-env-changed={key}");
        println!("cargo:rerun-if-changed={}", path.display());
    }
    let memory = format!(
        "MEMORY {{\n  FLASH : ORIGIN = {origin:#x}, LENGTH = {length:#x}\n  RAM : ORIGIN = 0x20000008, LENGTH = 0x3fff0\n}}\n",
        length = end - origin
    );
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    fs::write(out.join("memory.x"), memory).unwrap();
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rustc-link-arg=-Tlink.x");
    println!("cargo:rerun-if-env-changed=KB_FLASH_ORIGIN");
    println!("cargo:rerun-if-changed=build.rs");
}
