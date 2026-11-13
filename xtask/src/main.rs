mod heap;
mod probe;

use kb_component_build::{Contract, compile};
use std::{
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use xtask::{
    FirmwareImage, NOSD_APPLICATION_START, S140_V7_APPLICATION_START, STOCK_APPLICATION_START,
    targets,
};

#[global_allocator]
static ALLOCATOR: heap::TrackingAllocator = heap::TrackingAllocator;

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result {
    let args: Vec<_> = env::args().skip(1).collect();
    match args.as_slice() {
        [command, elf, output] if command == "uf2" => convert(elf, output, STOCK_APPLICATION_START),
        [command, elf, output, option] if command == "uf2" && option == "--nosd" => convert(elf, output, NOSD_APPLICATION_START),
        [command, elf, output, option] if command == "uf2" && option == "--s140v7" => convert(elf, output, S140_V7_APPLICATION_START),
        [command, name, options @ ..] if command == "build" => build(name, options),
        [command] if command == "check" => check(),
        [command, directory] if command == "verify-release" => {
            let names = fs::read_dir(directory)?
                .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
                .collect::<std::io::Result<Vec<_>>>()?;
            targets::verify_release_names(&names)?;
            println!("Release contains exactly the six supported UF2 images");
            Ok(())
        }
        [command] if command == "targets" => {
            for target in targets::TARGETS {
                println!("{}|{}|{}|{:#x}|{}", target.name, target.binary, target.features, target.flash_origin,
                    if target.flash_origin == S140_V7_APPLICATION_START { "--s140v7" } else { "" });
            }
            Ok(())
        }
        [command, contract, paths @ ..] if command == "probe" && !paths.is_empty() => {
            let contract = match contract.as_str() {
                "braille" => Contract::Braille,
                "keyboard" => Contract::Keyboard,
                _ => return Err("probe contract must be braille or keyboard".into()),
            };
            let paths = paths.iter().map(PathBuf::from).collect::<Vec<_>>();
            let peak = probe::check(contract, &paths, 160 * 1024)?;
            println!("Component probe passed: {peak} peak heap bytes");
            Ok(())
        }
        _ => Err("usage: cargo xtask targets\n       cargo xtask build TARGET [--source file.wlt] [--nosd|--s140v7|--s140v6]\n       cargo xtask uf2 firmware.elf output.uf2 [--nosd|--s140v7]\n       cargo xtask probe <braille|keyboard> entry.core.wasm [leaf.core.wasm]\n       cargo xtask verify-release DIRECTORY\n       cargo xtask check".into()),
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn checked(command: &mut Command) -> Result {
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("command failed with {status}: {command:?}").into())
    }
}

fn check() -> Result {
    checked(
        Command::new("cargo")
            .current_dir(root())
            .args(["test", "--workspace", "--locked"]),
    )?;
    checked(Command::new("cargo").current_dir(root()).args([
        "check",
        "--locked",
        "-p",
        "kb-core",
        "-p",
        "kb-component-runtime",
        "-p",
        "kb-display",
        "--target",
        "thumbv7em-none-eabihf",
    ]))
}

fn build(name: &str, options: &[String]) -> Result {
    let target = targets::find(name)
        .ok_or_else(|| format!("unknown firmware target: {name}; run cargo xtask targets"))?;
    let root = root();
    let mut source = root.join(target.source());
    let mut base = target.flash_origin;
    let mut options = options.iter();
    while let Some(option) = options.next() {
        match option.as_str() {
            "--source" => source = PathBuf::from(options.next().ok_or("--source requires a path")?),
            "--nosd" => base = NOSD_APPLICATION_START,
            "--s140v7" => base = S140_V7_APPLICATION_START,
            "--s140v6" => base = STOCK_APPLICATION_START,
            _ => return Err(format!("unsupported option for {name}: {option}").into()),
        }
    }
    let suffix = if base == NOSD_APPLICATION_START {
        "-nosd"
    } else if base == target.flash_origin {
        ""
    } else if base == S140_V7_APPLICATION_START {
        "-s140v7"
    } else {
        "-s140v6"
    };
    let artifact_name = format!("{name}{suffix}");
    let component_dir = root.join("target/components").join(&artifact_name);
    let artifacts = compile(&source.canonicalize()?, &component_dir, target.contract)?;
    let peak = probe::check(target.contract, &[artifacts.core_path.clone()], 160 * 1024)?;
    println!("Component probe passed: {peak}/163840 peak heap bytes");
    let firmware_target = root.join("firmware/nrf52840/target");
    checked(
        Command::new("cargo")
            .current_dir(root.join("firmware/nrf52840"))
            .args([
                "build",
                "--release",
                "--locked",
                "--target",
                "thumbv7em-none-eabihf",
                "--no-default-features",
                "--features",
                target.features,
                "--bin",
                target.binary,
            ])
            .env("CARGO_TARGET_DIR", &firmware_target)
            .env("KB_FLASH_ORIGIN", format!("{base:#x}"))
            .env(
                match target.contract {
                    Contract::Braille => "KB_BRAILLE_WASM",
                    Contract::Keyboard => "KB_KEYBOARD_WASM",
                },
                &artifacts.core_path,
            ),
    )?;
    let dist = root.join("dist");
    fs::create_dir_all(&dist)?;
    let elf = dist.join(format!("{artifact_name}.elf"));
    fs::copy(
        firmware_target
            .join("thumbv7em-none-eabihf/release")
            .join(target.binary),
        &elf,
    )?;
    fs::copy(
        artifacts.component_path,
        dist.join(format!("{artifact_name}.component.wasm")),
    )?;
    convert(&elf, dist.join(format!("{artifact_name}.uf2")), base)
}

fn convert(elf: impl AsRef<Path>, output: impl AsRef<Path>, base: u32) -> Result {
    let bytes = fs::read(elf)?;
    let firmware = FirmwareImage::from_elf(&bytes, base)?;
    let reserve = xtask::ram_reserve(&bytes)?;
    if reserve < 16 * 1024 {
        return Err(format!("firmware leaves {reserve} bytes of RAM for the native stack; at least 16384 are required").into());
    }
    let output = output.as_ref();
    if let Some(parent) = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(output, firmware.uf2()?)?;
    println!(
        "{}: {} flash bytes at {base:#x}; {reserve} bytes native RAM reserve",
        output.display(),
        firmware.data.len()
    );
    Ok(())
}
