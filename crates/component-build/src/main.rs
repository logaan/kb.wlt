use kb_component_build::{Contract, compile, compile_chain};
use std::path::Path;

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if let [command, contract, source, downstream, output] = args.as_slice() {
        if command == "chain" {
            let contract = match contract.as_str() {
                "led" => Contract::Led,
                "braille" => Contract::Braille,
                "keyboard" => Contract::Keyboard,
                _ => {
                    eprintln!("unknown contract");
                    std::process::exit(2);
                }
            };
            match compile_chain(
                &[Path::new(source), Path::new(downstream)],
                Path::new(output),
                contract,
            ) {
                Ok(artifacts) => {
                    println!("{}", artifacts.component_path.display());
                    for path in artifacts.core_paths {
                        println!("{}", path.display());
                    }
                }
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(1);
                }
            }
            return;
        }
    }
    let result = match args.as_slice() {
        [contract, source, output] => {
            let contract = match contract.as_str() {
                "led" => Contract::Led,
                "braille" => Contract::Braille,
                "keyboard" => Contract::Keyboard,
                _ => {
                    eprintln!("contract must be led, braille, or keyboard");
                    std::process::exit(2);
                }
            };
            compile(Path::new(source), Path::new(output), contract)
        }
        _ => {
            eprintln!(
                "usage: kb-component-build <led|braille|keyboard> <source.wlt> <output-directory>"
            );
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
