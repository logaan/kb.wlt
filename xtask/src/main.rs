use std::{env, error::Error, fs, path::Path};
use xtask::{FirmwareImage, NOSD_APPLICATION_START, STOCK_APPLICATION_START};

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    match args.as_slice() {
        [command, elf, output] if command == "uf2" => convert(elf, output, STOCK_APPLICATION_START),
        [command, elf, output, option] if command == "uf2" && option == "--nosd" => {
            convert(elf, output, NOSD_APPLICATION_START)
        }
        _ => Err("usage: cargo xtask uf2 <firmware.elf> <output.uf2> [--nosd]".into()),
    }
}

fn convert(elf: &str, output: &str, base: u32) -> Result<(), Box<dyn Error>> {
    let firmware = FirmwareImage::from_elf(&fs::read(elf)?, base)?;
    let uf2 = firmware.uf2()?;
    if let Some(parent) = Path::new(output)
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(output, uf2)?;
    println!("{output}: {} flash bytes at {base:#x}", firmware.data.len());
    Ok(())
}
