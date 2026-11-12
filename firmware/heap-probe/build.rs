use std::{env, fs, path::PathBuf};

fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    println!("cargo:rustc-link-search={}", root.display());
    for (key, stage) in [
        ("KB_KEYBOARD_STAGE0_WASM", "stage0"),
        ("KB_KEYBOARD_STAGE1_WASM", "stage1"),
    ] {
        let path = env::var_os(key).map(PathBuf::from).unwrap_or_else(|| {
            root.join("../../components/generated/keyboard-composed")
                .join(stage)
                .join("keyboard.core.wasm")
        });
        let path = fs::canonicalize(path).expect("WebAssembly component artifact is missing");
        println!("cargo:rustc-env={key}={}", path.display());
        println!("cargo:rerun-if-env-changed={key}");
        println!("cargo:rerun-if-changed={}", path.display());
    }
}
