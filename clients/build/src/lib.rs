mod bindings;

use std::{
    path::{Path, PathBuf},
    process::Command,
};
use wasmparser::{Parser, Payload, Validator};
use wit_component::DecodedWasm;
use wit_parser::{Resolve, Type, TypeDefKind, WorldItem, WorldKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Contract {
    Braille,
    Keyboard,
}

impl Contract {
    pub fn name(self) -> &'static str {
        match self {
            Self::Braille => "braille",
            Self::Keyboard => "keyboard",
        }
    }

    pub(crate) const fn functions(self) -> &'static [&'static str] {
        match self {
            Self::Braille => &["step", "map-cell"],
            Self::Keyboard => &["process", "transform-output"],
        }
    }

    pub fn interface(self) -> &'static str {
        match self {
            Self::Braille => "kb:firmware/braille@0.1.0",
            Self::Keyboard => "kb:firmware/keyboard@0.1.0",
        }
    }
}

#[derive(Debug)]
pub struct Artifacts {
    pub component_path: PathBuf,
    pub core_path: PathBuf,
}

pub fn compile(source: &Path, output_dir: &Path, contract: Contract) -> Result<Artifacts, String> {
    compile_stage(source, output_dir, contract, false, false)
}

pub fn compile_firmware(
    source: &Path,
    output_dir: &Path,
    contract: Contract,
) -> Result<Artifacts, String> {
    let artifacts = compile_stage(source, output_dir, contract, false, true)?;
    optimize(artifacts, contract, true)
}

pub fn compile_optimized(
    source: &Path,
    output_dir: &Path,
    contract: Contract,
) -> Result<Artifacts, String> {
    let artifacts = compile(source, output_dir, contract)?;
    optimize(artifacts, contract, false)
}

fn optimize(artifacts: Artifacts, contract: Contract, display: bool) -> Result<Artifacts, String> {
    let staging = tempfile::tempdir().map_err(|error| error.to_string())?;
    let optimized = staging.path().join("optimized.wasm");
    let binary = std::env::var_os("WASM_OPT").unwrap_or_else(|| "wasm-opt".into());
    let output = Command::new(binary)
        .arg(&artifacts.core_path)
        .args([
            "-O4",
            "--always-inline-max-function-size",
            "0",
            "--flexible-inline-max-function-size",
            "0",
            "--one-caller-inline-max-function-size",
            "0",
            "--enable-bulk-memory",
            "--enable-multivalue",
            "--enable-tail-call",
        ])
        .arg("-o")
        .arg(&optimized)
        .output()
        .map_err(|error| format!("failed to run Binaryen optimizer: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "core optimization failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        ));
    }
    let bytes = std::fs::read(&optimized).map_err(|error| error.to_string())?;
    Validator::new()
        .validate_all(&bytes)
        .map_err(|error| error.to_string())?;
    validate_core(&bytes, contract, false, display)?;
    std::fs::write(&artifacts.core_path, bytes).map_err(|error| error.to_string())?;
    Ok(artifacts)
}

fn compile_stage(
    source: &Path,
    output_dir: &Path,
    contract: Contract,
    imports: bool,
    display: bool,
) -> Result<Artifacts, String> {
    let staging = tempfile::tempdir().map_err(|e| e.to_string())?;
    let source_dir = staging.path().join("src");
    let wit_dir = staging.path().join("wit/deps");
    std::fs::create_dir_all(&source_dir).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&wit_dir).map_err(|e| e.to_string())?;
    let mut program =
        std::fs::read_to_string(source).map_err(|e| format!("{}: {e}", source.display()))?;
    if display {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../components");
        let mut append = |file: &str| -> Result<(), String> {
            let text =
                std::fs::read_to_string(root.join(file)).map_err(|e| format!("{file}: {e}"))?;
            let (_, definitions) = text.split_once('\n').ok_or("empty Wavelet source")?;
            program.push('\n');
            program.push_str(definitions);
            Ok(())
        };
        if contract == Contract::Keyboard {
            append("braille/braille.wlt")?;
        }
        append("display/display.wlt")?;
    }
    std::fs::write(source_dir.join("module.wlt"), program).map_err(|e| e.to_string())?;
    std::fs::write(
        wit_dir.join("firmware.wit"),
        include_str!("../../wit/firmware.wit"),
    )
    .map_err(|e| e.to_string())?;
    let binary = std::env::var_os("WAVELET").unwrap_or_else(|| "wavelet".into());
    let output = Command::new(binary)
        .arg("build")
        .arg(source_dir.join("module.wlt"))
        .arg("-o")
        .arg(staging.path().join("out"))
        .output()
        .map_err(|e| format!("failed to run Wavelet compiler: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "Wavelet compiler failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let entries = std::fs::read_dir(staging.path().join("out")).map_err(|e| e.to_string())?;
    let paths = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "wasm"))
        .collect::<Vec<_>>();
    if paths.len() != 1 {
        return Err("Wavelet must produce exactly one component".into());
    }
    let component = std::fs::read(&paths[0]).map_err(|e| e.to_string())?;
    let core = prepare_stage(&component, contract, imports, display)?;
    std::fs::create_dir_all(output_dir).map_err(|e| e.to_string())?;
    let artifacts = Artifacts {
        component_path: output_dir.join(format!("{}.component.wasm", contract.name())),
        core_path: output_dir.join(format!("{}.core.wasm", contract.name())),
    };
    std::fs::write(&artifacts.component_path, component).map_err(|e| e.to_string())?;
    std::fs::write(&artifacts.core_path, core).map_err(|e| e.to_string())?;
    Ok(artifacts)
}

pub fn prepare(component: &[u8], contract: Contract) -> Result<Vec<u8>, String> {
    prepare_stage(component, contract, false, false)
}

fn prepare_stage(
    component: &[u8],
    contract: Contract,
    imports: bool,
    display: bool,
) -> Result<Vec<u8>, String> {
    Validator::new()
        .validate_all(component)
        .map_err(|e| e.to_string())?;
    let DecodedWasm::Component(resolve, world) =
        wit_component::decode(component).map_err(|e| e.to_string())?
    else {
        return Err("expected a WebAssembly Component".into());
    };
    if imports {
        if resolve.worlds[world].imports.len() != 1 {
            return Err("middleware requires exactly one downstream interface".into());
        }
        for (key, item) in &resolve.worlds[world].imports {
            let WorldItem::Interface { id, .. } = item else {
                return Err("middleware imports must be interfaces".into());
            };
            let name = match key {
                WorldKey::Name(name) => name.clone(),
                WorldKey::Interface(_) => resolve.id_of(*id).unwrap_or_default(),
            };
            if name != contract.interface() {
                return Err("middleware imports an incompatible downstream interface".into());
            }
        }
    } else if !resolve.worlds[world].imports.is_empty() {
        return Err("embedded components must not require host imports".into());
    }
    let exports = &resolve.worlds[world].exports;
    for &(name, _) in interfaces(contract, display) {
        let interface = exports
            .iter()
            .find_map(|(key, item)| {
                let WorldItem::Interface { id, .. } = item else {
                    return None;
                };
                let actual = match key {
                    WorldKey::Name(name) => name.clone(),
                    WorldKey::Interface(id) => resolve.id_of(*id).unwrap_or_default(),
                };
                (actual == format!("kb:firmware/{name}@0.1.0")).then_some(*id)
            })
            .ok_or_else(|| format!("missing kb:firmware/{name}@0.1.0 export"))?;
        validate_interface(&resolve, interface, name)?;
    }
    bindings::validate(component, contract)?;
    let mut modules = Vec::new();
    for payload in Parser::new(0).parse_all(component) {
        if let Payload::ModuleSection {
            unchecked_range, ..
        } = payload.map_err(|e| e.to_string())?
        {
            modules.push(component[unchecked_range].to_vec());
        }
    }
    if modules.len() != if imports { 3 } else { 1 } {
        return Err("component contains unsupported core module topology".into());
    }
    let core = modules.remove(0);
    validate_core(&core, contract, imports, display)?;
    Ok(core)
}

fn validate_interface(
    resolve: &Resolve,
    interface: wit_parser::InterfaceId,
    name: &str,
) -> Result<(), String> {
    let mut expected = Resolve::new();
    let package = expected
        .push_str("firmware.wit", include_str!("../../wit/firmware.wit"))
        .map_err(|e| e.to_string())?;
    let expected_interface = expected.packages[package].interfaces[name];
    let actual_functions = &resolve.interfaces[interface].functions;
    let expected_functions = &expected.interfaces[expected_interface].functions;
    if actual_functions.len() != expected_functions.len() {
        return Err("component does not implement every contract function".into());
    }
    for (name, function) in expected_functions {
        let actual = actual_functions
            .get(name)
            .ok_or_else(|| format!("missing {name} function"))?;
        let signature = |resolve: &Resolve, function: &wit_parser::Function| -> Result<_, String> {
            Ok((
                function
                    .params
                    .iter()
                    .map(|param| type_shape(resolve, param.ty))
                    .collect::<Result<Vec<_>, _>>()?,
                function
                    .result
                    .map(|ty| type_shape(resolve, ty))
                    .transpose()?,
            ))
        };
        if signature(resolve, actual)? != signature(&expected, function)? {
            return Err(format!("incompatible {name} ABI"));
        }
    }
    Ok(())
}

fn type_shape(resolve: &Resolve, ty: Type) -> Result<String, String> {
    Ok(match ty {
        Type::String => "string".into(),
        Type::Bool => "bool".into(),
        Type::U8 => "u8".into(),
        Type::U16 => "u16".into(),
        Type::U32 => "u32".into(),
        Type::Id(id) => match &resolve.types[id].kind {
            TypeDefKind::Type(ty) => type_shape(resolve, *ty)?,
            TypeDefKind::Record(record) => format!(
                "{{{}}}",
                record
                    .fields
                    .iter()
                    .map(|field| Ok(format!("{}:{}", field.name, type_shape(resolve, field.ty)?)))
                    .collect::<Result<Vec<_>, String>>()?
                    .join(",")
            ),
            TypeDefKind::List(ty) => format!("list<{}>", type_shape(resolve, *ty)?),
            TypeDefKind::Option(ty) => format!("option<{}>", type_shape(resolve, *ty)?),
            TypeDefKind::Tuple(tuple) => format!(
                "tuple<{}>",
                tuple
                    .types
                    .iter()
                    .map(|ty| type_shape(resolve, *ty))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(",")
            ),
            TypeDefKind::Enum(enumeration) => format!(
                "enum<{}>",
                enumeration
                    .cases
                    .iter()
                    .map(|case| case.name.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            TypeDefKind::Variant(variant) => format!(
                "variant<{}>",
                variant
                    .cases
                    .iter()
                    .map(|case| Ok(format!(
                        "{}({})",
                        case.name,
                        case.ty
                            .map(|ty| type_shape(resolve, ty))
                            .transpose()?
                            .unwrap_or_default()
                    )))
                    .collect::<Result<Vec<_>, String>>()?
                    .join(",")
            ),
            _ => return Err("unsupported embedded ABI type".into()),
        },
        _ => return Err("unsupported embedded ABI scalar".into()),
    })
}

pub(crate) fn interfaces(
    contract: Contract,
    display: bool,
) -> &'static [(&'static str, &'static [&'static str])] {
    const KEYBOARD: (&str, &[&str]) = ("keyboard", Contract::Keyboard.functions());
    const BRAILLE: (&str, &[&str]) = ("braille", Contract::Braille.functions());
    const DISPLAY: (&str, &[&str]) = ("display", &["render", "navigate"]);
    match (contract, display) {
        (Contract::Keyboard, true) => &[KEYBOARD, BRAILLE, DISPLAY],
        (Contract::Braille, true) => &[BRAILLE, DISPLAY],
        (Contract::Keyboard, false) => &[KEYBOARD],
        (Contract::Braille, false) => &[BRAILLE],
    }
}

fn validate_core(
    core: &[u8],
    contract: Contract,
    imports: bool,
    display: bool,
) -> Result<(), String> {
    let mut memories = 0;
    let mut exports = Vec::new();
    for payload in Parser::new(0).parse_all(core) {
        match payload.map_err(|e| e.to_string())? {
            Payload::ImportSection(section) => {
                for import in section.into_imports() {
                    let import = import.map_err(|e| e.to_string())?;
                    if !imports
                        || import.module != contract.interface()
                        || !matches!(import.name, "step" | "map-cell")
                        || !matches!(import.ty, wasmparser::TypeRef::Func(_))
                    {
                        return Err("unsupported embedded core import".into());
                    }
                }
            }
            Payload::StartSection { .. } => {
                return Err("embedded core start functions are unsupported".into());
            }
            Payload::MemorySection(section) => {
                for memory in section {
                    let memory = memory.map_err(|e| e.to_string())?;
                    memories += 1;
                    let allowed_pages = if display && contract == Contract::Keyboard {
                        2
                    } else {
                        1
                    };
                    if memory.initial > allowed_pages || memory.memory64 || memory.shared {
                        return Err("embedded component requires unsupported memory".into());
                    }
                }
            }
            Payload::ExportSection(section) => {
                for export in section {
                    exports.push(export.map_err(|e| e.to_string())?.name.to_owned());
                }
            }
            _ => {}
        }
    }
    if memories != 1 {
        return Err("embedded component requires exactly one memory".into());
    }
    if !exports.iter().any(|name| name == "memory") {
        return Err("missing canonical memory export".into());
    }
    for &(name, functions) in interfaces(contract, display) {
        for function in functions {
            let export = format!("kb:firmware/{name}@0.1.0#{function}");
            if !exports.contains(&export) || !exports.contains(&format!("cabi_post_{export}")) {
                return Err(format!(
                    "missing canonical export {export} or its post-return"
                ));
            }
        }
    }

    Ok(())
}

#[derive(Debug)]
pub struct ChainArtifacts {
    pub component_path: PathBuf,
    pub core_paths: Vec<PathBuf>,
}

pub fn compile_chain(
    sources: &[&Path],
    output_dir: &Path,
    contract: Contract,
) -> Result<ChainArtifacts, String> {
    if contract == Contract::Keyboard {
        return Err("keyboard components compose through the host middleware pipeline".into());
    }
    if sources.len() != 2 {
        return Err("embedded middleware chains require exactly two components".into());
    }
    let mut stages = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        let mut stage = compile_stage(
            source,
            &output_dir.join(format!("stage{index}")),
            contract,
            index + 1 < sources.len(),
            false,
        )?;
        let renamed = output_dir.join(format!("stage{index}-{}.component.wasm", contract.name()));
        std::fs::rename(&stage.component_path, &renamed).map_err(|e| e.to_string())?;
        stage.component_path = renamed;
        stages.push(stage);
    }
    let component_path = output_dir.join(format!("{}.component.wasm", contract.name()));
    let binary = std::env::var_os("WAVELET").unwrap_or_else(|| "wavelet".into());
    let output = Command::new(binary)
        .arg("compose")
        .args(stages.iter().map(|stage| &stage.component_path))
        .arg("-o")
        .arg(&component_path)
        .output()
        .map_err(|e| format!("failed to compose components: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "component composition failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let bytes = std::fs::read(&component_path).map_err(|e| e.to_string())?;
    Validator::new()
        .validate_all(&bytes)
        .map_err(|e| e.to_string())?;
    let DecodedWasm::Component(resolve, world) =
        wit_component::decode(&bytes).map_err(|e| e.to_string())?
    else {
        return Err("composition did not produce a component".into());
    };
    if !resolve.worlds[world].imports.is_empty() {
        return Err("composition contains unresolved imports".into());
    }
    Ok(ChainArtifacts {
        component_path,
        core_paths: stages.into_iter().map(|stage| stage.core_path).collect(),
    })
}
