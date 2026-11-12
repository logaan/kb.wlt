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
};

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
        _ => Err("usage: cargo xtask build <xiao-led|paintbrush-usb|paintbrush-ble|paintbrush-display> [--source file.wlt] [--nosd] [--left]\n       cargo xtask uf2 <firmware.elf> <output.uf2> [--nosd]\n       cargo xtask check".into()),
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
            .args(["test", "--workspace"]),
    )?;
    checked(Command::new("cargo").current_dir(root()).args([
        "check",
        "-p",
        "kb-core",
        "-p",
        "kb-component-runtime",
        "--target",
        "thumbv7em-none-eabihf",
    ]))
}

fn build(name: &str, options: &[String]) -> Result {
    let (binary, contract, mut features) = match name {
        "xiao-led" => ("xiao-led", Contract::Led, "usb".to_string()),
        "paintbrush-usb" => ("paintbrush-usb", Contract::Braille, "usb".to_string()),
        "paintbrush-ble" => ("paintbrush-ble", Contract::Braille, "ble".to_string()),
        "paintbrush-ble-display" => (
            "paintbrush-ble",
            Contract::Braille,
            "ble,display".to_string(),
        ),
        "paintbrush-satellite" => (
            "paintbrush-satellite",
            Contract::Braille,
            "split".to_string(),
        ),
        "paintbrush-left-satellite" => (
            "paintbrush-satellite",
            Contract::Braille,
            "split,satellite-left".to_string(),
        ),
        "paintbrush-right-satellite" => (
            "paintbrush-satellite",
            Contract::Braille,
            "split,satellite-right".to_string(),
        ),
        "paintbrush-central" => (
            "paintbrush-central",
            Contract::Braille,
            "usb,split".to_string(),
        ),
        "prospector-single" => (
            "prospector-dongle",
            Contract::Braille,
            "usb,split,prospector-display".to_string(),
        ),
        "prospector-pair" => (
            "prospector-dongle",
            Contract::Braille,
            "usb,split,split-pair,prospector-display".to_string(),
        ),
        "paintbrush-display" => (
            "paintbrush-usb",
            Contract::Braille,
            "usb,display".to_string(),
        ),
        _ => return Err(format!("unknown firmware target: {name}").into()),
    };
    let root = root();
    let mut source = root.join(format!("components/{0}/{0}.wlt", contract.name()));
    let default_base = if name.starts_with("xiao-") || name.starts_with("prospector-") {
        S140_V7_APPLICATION_START
    } else {
        STOCK_APPLICATION_START
    };
    let mut base = default_base;
    let mut left = false;
    let mut options = options.iter();
    while let Some(option) = options.next() {
        match option.as_str() {
            "--source" => source = PathBuf::from(options.next().ok_or("--source requires a path")?),
            "--nosd" => base = NOSD_APPLICATION_START,
            "--s140v7" => base = S140_V7_APPLICATION_START,
            "--s140v6" => base = STOCK_APPLICATION_START,
            "--left" if name.starts_with("paintbrush-") => {
                features.push_str(",paintbrush-left");
                left = true;
            }
            _ => return Err(format!("unknown option: {option}").into()),
        }
    }
    let source = source.canonicalize()?;
    let suffix = if base == NOSD_APPLICATION_START {
        "-nosd"
    } else if base != default_base {
        if base == S140_V7_APPLICATION_START {
            "-s140v7"
        } else {
            "-s140v6"
        }
    } else {
        ""
    };
    let artifact_name = format!("{name}{}{suffix}", if left { "-left" } else { "" });
    let component_dir = root.join("target/components").join(&artifact_name);
    let artifacts = compile(&source, &component_dir, contract)?;
    let mut command = Command::new("cargo");
    command
        .current_dir(root.join("firmware/nrf52840"))
        .args([
            "build",
            "--release",
            "--locked",
            "--target",
            "thumbv7em-none-eabihf",
            "--no-default-features",
            "--features",
            &features,
            "--bin",
            binary,
        ])
        .env("KB_FLASH_ORIGIN", format!("{base:#x}"))
        .env(
            match contract {
                Contract::Led => "KB_LED_WASM",
                Contract::Braille => "KB_BRAILLE_WASM",
                Contract::Keyboard => "KB_KEYBOARD_WASM",
            },
            &artifacts.core_path,
        );
    checked(&mut command)?;
    let build_elf = root
        .join("firmware/nrf52840/target/thumbv7em-none-eabihf/release")
        .join(binary);
    let dist = root.join("dist");
    fs::create_dir_all(&dist)?;
    let elf = dist.join(format!("{artifact_name}.elf"));
    fs::copy(&build_elf, &elf)?;
    fs::copy(
        &artifacts.component_path,
        dist.join(format!("{artifact_name}.component.wasm")),
    )?;
    convert(&elf, &dist.join(format!("{artifact_name}.uf2")), base)
}

fn convert(elf: impl AsRef<Path>, output: impl AsRef<Path>, base: u32) -> Result {
    let firmware = FirmwareImage::from_elf(&fs::read(elf)?, base)?;
    let uf2 = firmware.uf2()?;
    let output = output.as_ref();
    if let Some(parent) = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(output, uf2)?;
    println!(
        "{}: {} flash bytes at {base:#x}",
        output.display(),
        firmware.data.len()
    );
    Ok(())
}
