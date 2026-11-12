mod heap;
mod probe;

use kb_component_build::{Contract, compile, compile_chain, compile_optimized};
use std::{
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
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
        [command] if command == "probe-arm" => arm_probe(&[
            root().join("components/generated/keyboard-composed/stage0/keyboard.core.wasm"),
            root().join("components/generated/keyboard-composed/stage1/keyboard.core.wasm"),
        ]),
        [command, first, second] if command == "probe-arm" => arm_probe(&[PathBuf::from(first), PathBuf::from(second)]),
        [command] if command == "targets" => {
            for target in targets::TARGETS {
                println!("{}|{}|{}|{:#x}|{}", target.name, target.binary, target.features, target.flash_origin,
                    if target.flash_origin == S140_V7_APPLICATION_START { "--s140v7" } else { "" });
            }
            Ok(())
        }
        [command, contract, paths @ ..] if command == "probe" && !paths.is_empty() => {
            let contract = match contract.as_str() {
                "led" => Contract::Led, "braille" => Contract::Braille, "keyboard" => Contract::Keyboard,
                _ => return Err("probe contract must be led, braille, or keyboard".into()),
            };
            let paths = paths.iter().map(PathBuf::from).collect::<Vec<_>>();
            let peak = probe::check(contract, &paths, if paths.len() == 1 { 160 } else { 208 } * 1024)?;
            println!("Component probe passed: {peak} peak heap bytes");
            Ok(())
        }
        _ => Err("usage: cargo xtask targets\n       cargo xtask build TARGET [--source file.wlt] [--middleware file.wlt] [--nosd|--s140v7|--s140v6] [--left]\n       cargo xtask uf2 firmware.elf output.uf2 [--nosd|--s140v7]\n       cargo xtask probe <led|braille|keyboard> entry.core.wasm [leaf.core.wasm]\n       cargo xtask probe-arm [stage0.core.wasm stage1.core.wasm]\n       cargo xtask check".into()),
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

fn arm_probe(paths: &[PathBuf; 2]) -> Result {
    let probe = root().join("firmware/heap-probe");
    let target = probe.join("target");
    checked(
        Command::new("cargo")
            .current_dir(&probe)
            .args([
                "build",
                "--locked",
                "--release",
                "--target",
                "thumbv7em-none-eabihf",
            ])
            .env("CARGO_TARGET_DIR", &target)
            .env("KB_KEYBOARD_STAGE0_WASM", paths[0].canonicalize()?)
            .env("KB_KEYBOARD_STAGE1_WASM", paths[1].canonicalize()?),
    )?;
    let executable = env::var_os("QEMU_SYSTEM_ARM").unwrap_or_else(|| "qemu-system-arm".into());
    let mut child = Command::new(executable)
        .args([
            "-machine",
            "mps2-an386",
            "-cpu",
            "cortex-m4",
            "-nographic",
            "-semihosting-config",
            "enable=on,target=native",
            "-kernel",
        ])
        .arg(target.join("thumbv7em-none-eabihf/release/kb-heap-probe"))
        .spawn()
        .map_err(|error| format!("ARM heap probe requires qemu-system-arm: {error}"))?;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait()? {
            return if status.success() {
                Ok(())
            } else {
                Err(format!("ARM heap probe failed with {status}").into())
            };
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("ARM heap probe exceeded 30 seconds".into());
        }
        std::thread::sleep(Duration::from_millis(50));
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
    let mut source = target.source().map(|path| root.join(path));
    let mut middleware = root.join(if target.keyboard_composed() {
        "components/shift/shift.wlt"
    } else {
        "components/led-dim/led-dim.wlt"
    });
    let mut features = target.features.to_string();
    let mut base = target.flash_origin;
    let mut left = false;
    let mut options = options.iter();
    while let Some(option) = options.next() {
        match option.as_str() {
            "--source" if target.contract.is_some() => {
                source = Some(PathBuf::from(
                    options.next().ok_or("--source requires a path")?,
                ))
            }
            "--middleware" if name == "xiao-led-composed" || target.keyboard_composed() => {
                middleware = PathBuf::from(options.next().ok_or("--middleware requires a path")?)
            }
            "--nosd" => base = NOSD_APPLICATION_START,
            "--s140v7" => base = S140_V7_APPLICATION_START,
            "--s140v6" => base = STOCK_APPLICATION_START,
            "--left" if name.starts_with("paintbrush-") => {
                features.push_str(",paintbrush-left");
                left = true;
            }
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
    let artifact_name = format!("{name}{}{suffix}", if left { "-left" } else { "" });
    let component_dir = root.join("target/components").join(&artifact_name);
    let mut command = Command::new("cargo");
    let firmware_target = root.join("firmware/nrf52840/target");
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
            target.binary,
        ])
        .env("CARGO_TARGET_DIR", &firmware_target)
        .env("KB_FLASH_ORIGIN", format!("{base:#x}"));
    let mut component_paths = Vec::new();
    if let (Some(contract), Some(source)) = (target.contract, source) {
        let source = source.canonicalize()?;
        let core_paths = if target.keyboard_composed() {
            let first = compile_optimized(
                &middleware.canonicalize()?,
                &component_dir.join("stage0"),
                contract,
            )?;
            let second = compile_optimized(&source, &component_dir.join("stage1"), contract)?;
            component_paths.extend([first.component_path, second.component_path]);
            command
                .env("KB_KEYBOARD_STAGE0_WASM", &first.core_path)
                .env("KB_KEYBOARD_STAGE1_WASM", &second.core_path);
            vec![first.core_path, second.core_path]
        } else if name == "xiao-led-composed" {
            let artifacts = compile_chain(
                &[&middleware.canonicalize()?, &source],
                &component_dir,
                contract,
            )?;
            component_paths.push(artifacts.component_path);
            command.env("KB_LED_WRAPPER_WASM", &artifacts.core_paths[0]);
            artifacts.core_paths
        } else {
            let artifacts = compile(&source, &component_dir, contract)?;
            component_paths.push(artifacts.component_path);
            vec![artifacts.core_path]
        };
        if target.keyboard_composed() {
            arm_probe(&[core_paths[0].clone(), core_paths[1].clone()])?;
        } else {
            let budget = if core_paths.len() > 1 { 208 } else { 160 } * 1024;
            let peak = probe::check(contract, &core_paths, budget)?;
            println!("Component probe passed: {peak}/{budget} peak heap bytes");
        }
        command.env(
            match contract {
                Contract::Led => "KB_LED_WASM",
                Contract::Braille => "KB_BRAILLE_WASM",
                Contract::Keyboard => "KB_KEYBOARD_WASM",
            },
            core_paths.last().unwrap(),
        );
    }
    checked(&mut command)?;
    let build_elf = firmware_target
        .join("thumbv7em-none-eabihf/release")
        .join(target.binary);
    let dist = root.join("dist");
    fs::create_dir_all(&dist)?;
    let elf = dist.join(format!("{artifact_name}.elf"));
    fs::copy(&build_elf, &elf)?;
    for (index, component) in component_paths.iter().enumerate() {
        let stage = if component_paths.len() > 1 {
            format!(".stage{index}")
        } else {
            String::new()
        };
        fs::copy(
            component,
            dist.join(format!("{artifact_name}{stage}.component.wasm")),
        )?;
    }
    convert(&elf, &dist.join(format!("{artifact_name}.uf2")), base)
}

fn convert(elf: impl AsRef<Path>, output: impl AsRef<Path>, base: u32) -> Result {
    let bytes = fs::read(elf)?;
    let firmware = FirmwareImage::from_elf(&bytes, base)?;
    let reserve = xtask::ram_reserve(&bytes)?;
    if reserve < 16 * 1024 {
        return Err(format!("firmware leaves {reserve} bytes of RAM for the native stack; at least 16384 are required").into());
    }
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
        "{}: {} flash bytes at {base:#x}; {reserve} bytes native RAM reserve",
        output.display(),
        firmware.data.len()
    );
    Ok(())
}
