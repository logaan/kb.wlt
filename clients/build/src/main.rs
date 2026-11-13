use kb_component_build::{Contract, compile, compile_chain, compile_optimized};
use std::path::Path;

fn contract(name: &str) -> Result<Contract, String> {
    match name {
        "braille" => Ok(Contract::Braille),
        "keyboard" => Ok(Contract::Keyboard),
        _ => Err("contract must be braille or keyboard".into()),
    }
}

fn run(args: &[String]) -> Result<(), String> {
    if let [command, name, source, downstream, output] = args {
        if command == "chain" {
            let artifacts = compile_chain(
                &[Path::new(source), Path::new(downstream)],
                Path::new(output),
                contract(name)?,
            )?;
            println!("{}", artifacts.component_path.display());
            for path in artifacts.core_paths {
                println!("{}", path.display());
            }
            return Ok(());
        }
    }
    let artifacts = match args {
        [command, name, source, output] if command == "optimized" => compile_optimized(Path::new(source), Path::new(output), contract(name)?)?,
        [name, source, output] => compile(Path::new(source), Path::new(output), contract(name)?)?,
        _ => return Err("usage: kb-component-build [optimized] <braille|keyboard> <source.wlt> <output-directory>\n       kb-component-build chain <braille|keyboard> <source.wlt> <downstream.wlt> <output-directory>".into()),
    };
    println!(
        "{}\n{}",
        artifacts.component_path.display(),
        artifacts.core_path.display()
    );
    Ok(())
}

fn main() {
    if let Err(error) = run(&std::env::args().skip(1).collect::<Vec<_>>()) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
