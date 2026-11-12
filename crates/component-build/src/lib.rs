use std::{
    path::{Path, PathBuf},
    process::Command,
};
use wasmparser::{Parser, Payload, Validator};
use wit_component::DecodedWasm;
use wit_parser::{Resolve, Type, TypeDefKind, WorldItem, WorldKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Contract {
    Led,
    Braille,
}

impl Contract {
    pub fn name(self) -> &'static str {
        match self {
            Self::Led => "led",
            Self::Braille => "braille",
        }
    }

    pub fn interface(self) -> &'static str {
        match self {
            Self::Led => "kb:firmware/led@0.1.0",
            Self::Braille => "kb:firmware/braille@0.1.0",
        }
    }
}

#[derive(Debug)]
pub struct Artifacts {
    pub component_path: PathBuf,
    pub core_path: PathBuf,
}

pub fn compile(source: &Path, output_dir: &Path, contract: Contract) -> Result<Artifacts, String> {
    let staging = tempfile::tempdir().map_err(|e| e.to_string())?;
    let source_dir = staging.path().join("src");
    let wit_dir = staging.path().join("wit/deps");
    std::fs::create_dir_all(&source_dir).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&wit_dir).map_err(|e| e.to_string())?;
    std::fs::copy(source, source_dir.join("module.wlt"))
        .map_err(|e| format!("{}: {e}", source.display()))?;
    std::fs::write(
        wit_dir.join("firmware.wit"),
        include_str!("../../../wit/firmware.wit"),
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
    let core = prepare(&component, contract)?;
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
    Validator::new()
        .validate_all(component)
        .map_err(|e| e.to_string())?;
    let DecodedWasm::Component(resolve, world) =
        wit_component::decode(component).map_err(|e| e.to_string())?
    else {
        return Err("expected a WebAssembly Component".into());
    };
    if !resolve.worlds[world].imports.is_empty() {
        return Err("embedded components must not require host imports".into());
    }
    let interface = resolve.worlds[world]
        .exports
        .iter()
        .find_map(|(key, item)| {
            let WorldItem::Interface { id, .. } = item else {
                return None;
            };
            let name = match key {
                WorldKey::Name(name) => name.clone(),
                WorldKey::Interface(id) => resolve.id_of(*id).unwrap_or_default(),
            };
            (name == contract.interface()).then_some(*id)
        })
        .ok_or_else(|| format!("missing {} export", contract.interface()))?;
    let function = resolve.interfaces[interface]
        .functions
        .get("step")
        .ok_or("missing step function")?;
    let shape = function
        .params
        .iter()
        .map(|param| type_shape(&resolve, param.ty))
        .collect::<Result<Vec<_>, _>>()?;
    let result = function
        .result
        .map(|ty| type_shape(&resolve, ty))
        .transpose()?
        .ok_or("step must return a value")?;
    let (expected_params, expected_result) = match contract {
        Contract::Led => (
            vec![
                "{phase:u8,speed:u8,last-change-ms:u32,button-was-down:bool}",
                "u32",
                "bool",
            ],
            "{state:{phase:u8,speed:u8,last-change-ms:u32,button-was-down:bool},red:u8,green:u8,blue:u8}",
        ),
        Contract::Braille => (
            vec!["{held:u16,chord:u16}", "u16"],
            "{state:{held:u16,chord:u16},usage:u8,modifiers:u8}",
        ),
    };
    if shape != expected_params || result != expected_result {
        return Err(format!(
            "incompatible {} ABI: {shape:?} -> {result}",
            contract.interface()
        ));
    }
    let mut modules = Vec::new();
    for payload in Parser::new(0).parse_all(component) {
        if let Payload::ModuleSection {
            unchecked_range, ..
        } = payload.map_err(|e| e.to_string())?
        {
            modules.push(component[unchecked_range].to_vec());
        }
    }
    if modules.len() != 1 {
        return Err("embedded component must contain exactly one core module".into());
    }
    let core = modules.pop().unwrap();
    validate_core(&core, contract)?;
    Ok(core)
}

fn type_shape(resolve: &Resolve, ty: Type) -> Result<String, String> {
    Ok(match ty {
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
            _ => return Err("unsupported embedded ABI type".into()),
        },
        _ => return Err("unsupported embedded ABI scalar".into()),
    })
}

fn validate_core(core: &[u8], contract: Contract) -> Result<(), String> {
    let mut memories = 0;
    let mut exports = Vec::new();
    for payload in Parser::new(0).parse_all(core) {
        match payload.map_err(|e| e.to_string())? {
            Payload::ImportSection(imports) if imports.count() != 0 => {
                return Err("embedded core module imports are unsupported".into());
            }
            Payload::StartSection { .. } => {
                return Err("embedded core start functions are unsupported".into());
            }
            Payload::MemorySection(section) => {
                for memory in section {
                    let memory = memory.map_err(|e| e.to_string())?;
                    memories += 1;
                    if memory.initial > 1 || memory.memory64 || memory.shared {
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
    for export in [
        "memory".into(),
        format!("{}#step", contract.interface()),
        format!("cabi_post_{}#step", contract.interface()),
    ] {
        if !exports.contains(&export) {
            return Err(format!("missing canonical export {export}"));
        }
    }
    Ok(())
}
