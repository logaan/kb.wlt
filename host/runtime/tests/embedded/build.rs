use std::{env, fs, path::PathBuf};

#[path = "../../../../clients/build/embedded/mod.rs"]
mod guest;

fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let keyboard = env::var_os("CARGO_FEATURE_KEYBOARD").is_some();
    let component = if keyboard {
        "toucan-keymap/keyboard.core.wasm"
    } else {
        "braille.core.wasm"
    };
    guest::compile(
        &root
            .join("../../../../clients/components/generated")
            .join(component),
        &out,
        &root.join("../../embedded"),
        keyboard,
    );
    fs::copy(root.join("memory.x"), out.join("memory.x")).unwrap();
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rustc-link-arg=-Tlink.x");
}
