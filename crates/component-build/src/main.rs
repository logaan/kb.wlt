use kb_component_build::{Contract, compile};
use std::path::Path;

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let result = match args.as_slice() {
        [contract, source, output] => {
            let contract = match contract.as_str() {
                "led" => Contract::Led,
                "braille" => Contract::Braille,
                _ => {
                    eprintln!("contract must be led or braille");
                    std::process::exit(2);
                }
            };
            compile(Path::new(source), Path::new(output), contract)
        }
        _ => {
            eprintln!("usage: kb-component-build <led|braille> <source.wlt> <output-directory>");
            std::process::exit(2);
        }
    };
    match result {
        Ok(artifacts) => println!(
            "{}\n{}",
            artifacts.component_path.display(),
            artifacts.core_path.display()
        ),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
