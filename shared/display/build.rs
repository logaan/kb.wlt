use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

fn add_sources(build: &mut cc::Build, directory: &Path) {
    let mut paths: Vec<_> = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    paths.sort_unstable();
    for path in paths {
        if path.is_dir() {
            add_sources(build, &path);
        } else if path.extension().is_some_and(|extension| extension == "c") {
            build.file(path);
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=lvgl");
    let tft = env::var_os("CARGO_FEATURE_LVGL_TFT").is_some();
    let mip = env::var_os("CARGO_FEATURE_LVGL_MIP").is_some();
    if !tft && !mip {
        return;
    }

    let archive = "lvgl/lvgl-9.2.2.tar.gz";
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("lvgl");
    if output.exists() {
        fs::remove_dir_all(&output).unwrap();
    }
    fs::create_dir(&output).unwrap();
    let status = Command::new("tar")
        .args(["-xzf", archive, "-C"])
        .arg(&output)
        .status()
        .expect("cannot launch tar to unpack the vendored LVGL sources");
    assert!(status.success(), "cannot unpack the vendored LVGL sources");

    let target = env::var("TARGET").unwrap();
    let mut build = cc::Build::new();
    if target.ends_with("-none-eabihf") || target == "wasm32-unknown-unknown" {
        println!("cargo:rerun-if-env-changed=CLANG_PATH");
        let compiler = env::var_os(format!("CC_{}", target.replace('-', "_")))
            .or_else(|| env::var_os("CC"))
            .or_else(|| env::var_os("CLANG_PATH"))
            .unwrap_or_else(|| "clang".into());
        build
            .compiler(compiler)
            .flag("-ffreestanding")
            .define("KB_LVGL_FREESTANDING", None);
    }
    if target != env::var("HOST").unwrap() {
        if let Some(archiver) = rust_archiver(&target) {
            build.archiver(archiver);
            build.ar_flag("--format=gnu");
        }
    }
    build
        .include("lvgl")
        .include(&output)
        .define("LV_CONF_INCLUDE_SIMPLE", None)
        .opt_level_str("s")
        .warnings(false)
        .file("lvgl/bridge.c");
    if tft {
        build.define("KB_LVGL_TFT", None);
    }
    if mip {
        build.define("KB_LVGL_MIP", None);
    }
    add_sources(&mut build, &output.join("src"));
    build.compile("kb_lvgl");
}

fn rust_archiver(target: &str) -> Option<PathBuf> {
    if !env::var("HOST").ok()?.ends_with("-apple-darwin")
        || env::var_os("AR").is_some()
        || env::var_os(format!("AR_{}", target.replace('-', "_"))).is_some()
    {
        return None;
    }
    let rustc = env::var_os("RUSTC")?;
    let sysroot = Command::new(rustc)
        .args(["--print", "sysroot"])
        .output()
        .ok()?;
    let directory = String::from_utf8(sysroot.stdout).ok()?;
    let archiver = PathBuf::from(directory.trim())
        .join("lib/rustlib")
        .join(env::var("HOST").ok()?)
        .join("bin/llvm-ar");
    archiver.is_file().then_some(archiver)
}
