use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use wasm_encoder::{
    CodeSection, ConstExpr, Encode, ExportKind, ExportSection, GlobalSection, GlobalType,
    Instruction, Module, RawSection, ValType,
};
use wasmparser::{FunctionBody, Operator, Parser, Payload, Validator};

const ABI: &[(&str, usize, usize)] = &[
    ("cabi_realloc", 4, 1),
    ("kb:firmware/braille@0.1.0#map-cell", 1, 1),
    ("kb:firmware/braille@0.1.0#step", 3, 1),
    ("kb:firmware/display@0.1.0#render", 6, 1),
    ("kb:firmware/display@0.1.0#navigate", 4, 1),
    ("cabi_post_kb:firmware/braille@0.1.0#map-cell", 1, 0),
    ("cabi_post_kb:firmware/braille@0.1.0#step", 1, 0),
    ("cabi_post_kb:firmware/display@0.1.0#render", 1, 0),
    ("cabi_post_kb:firmware/display@0.1.0#navigate", 1, 0),
];

const KEYBOARD_ABI: &[(&str, usize, usize)] = &[
    ("kb:firmware/keyboard@0.1.0#process", 5, 1),
    ("kb:firmware/keyboard@0.1.0#transform-output", 4, 1),
    ("cabi_post_kb:firmware/keyboard@0.1.0#process", 1, 0),
    (
        "cabi_post_kb:firmware/keyboard@0.1.0#transform-output",
        1,
        0,
    ),
];

fn inject_fuel(bytes: &[u8], keyboard: bool) -> Result<Vec<u8>, String> {
    let types = Validator::new()
        .validate_all(bytes)
        .map_err(|error| error.to_string())?;
    let mut memory_count = 0;
    let mut result = Module::new();
    let mut code = None;
    let mut global_index = None;
    for payload in Parser::new(0).parse_all(bytes) {
        let payload = payload.map_err(|error| error.to_string())?;
        match payload {
            Payload::ImportSection(reader) if reader.count() != 0 => {
                return Err("the embedded module cannot import functions, tables or memory".into());
            }
            Payload::MemorySection(reader) => {
                if reader.count() != 1 {
                    return Err("embedded Wasm must have exactly one memory".into());
                }
                let range = reader.range();
                let allowed_pages = if keyboard { 2 } else { 1 };
                for memory in reader {
                    let memory = memory.map_err(|error| error.to_string())?;
                    if memory.memory64
                        || memory.shared
                        || memory.page_size_log2.is_some()
                        || !(1..=allowed_pages).contains(&memory.initial)
                    {
                        return Err(
                            "embedded Wasm exceeds its permitted initial linear memory".into()
                        );
                    }
                }
                memory_count += 1;
                result.section(&RawSection {
                    id: 5,
                    data: &bytes[range],
                });
            }
            Payload::TableSection(reader) => {
                if reader.count() > 1 {
                    return Err("embedded Wasm supports a single function table".into());
                }
                let range = reader.range();
                for table in reader {
                    let table = table.map_err(|error| error.to_string())?.ty;
                    if table.element_type != wasmparser::RefType::FUNCREF
                        || table.initial > 128
                        || table.table64
                        || table.shared
                    {
                        return Err("embedded Wasm requires a bounded function table".into());
                    }
                }
                result.section(&RawSection {
                    id: 4,
                    data: &bytes[range],
                });
            }
            Payload::GlobalSection(reader) => {
                let count = reader.count();
                let range = reader.range();
                let mut slice = wasmparser::BinaryReader::new(&bytes[range.clone()], range.start);
                slice.read_var_u32().map_err(|error| error.to_string())?;
                let mut data = vec![];
                (count + 1).encode(&mut data);
                data.extend_from_slice(&bytes[slice.original_position()..range.end]);
                GlobalType {
                    val_type: ValType::I32,
                    mutable: true,
                    shared: false,
                }
                .encode(&mut data);
                ConstExpr::i32_const(0).encode(&mut data);
                result.section(&RawSection { id: 6, data: &data });
                global_index = Some(count);
            }
            Payload::ExportSection(reader) => {
                let index = match global_index {
                    Some(index) => index,
                    None => {
                        let mut globals = GlobalSection::new();
                        globals.global(
                            GlobalType {
                                val_type: ValType::I32,
                                mutable: true,
                                shared: false,
                            },
                            &ConstExpr::i32_const(0),
                        );
                        result.section(&globals);
                        global_index = Some(0);
                        0
                    }
                };
                let old = reader
                    .into_iter()
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| error.to_string())?;
                if !old.iter().any(|export| {
                    export.name == "memory" && export.kind == wasmparser::ExternalKind::Memory
                }) {
                    return Err("missing embedded memory export".into());
                }
                for &(name, inputs, outputs) in ABI
                    .iter()
                    .chain(keyboard.then_some(KEYBOARD_ABI).into_iter().flatten())
                {
                    let export = old
                        .iter()
                        .find(|export| {
                            export.name == name && export.kind == wasmparser::ExternalKind::Func
                        })
                        .ok_or_else(|| format!("missing embedded function {name}"))?;
                    let ty = types[types.as_ref().core_function_at(export.index)].unwrap_func();
                    if ty.params() != &[wasmparser::ValType::I32; 6][..inputs]
                        || ty.results() != &[wasmparser::ValType::I32; 1][..outputs]
                    {
                        return Err(format!("incorrect embedded function signature for {name}"));
                    }
                }
                let mut exports = ExportSection::new();
                for export in old {
                    let kind = match export.kind {
                        wasmparser::ExternalKind::Func | wasmparser::ExternalKind::FuncExact => {
                            ExportKind::Func
                        }
                        wasmparser::ExternalKind::Table => ExportKind::Table,
                        wasmparser::ExternalKind::Memory => ExportKind::Memory,
                        wasmparser::ExternalKind::Global => ExportKind::Global,
                        wasmparser::ExternalKind::Tag => ExportKind::Tag,
                    };
                    exports.export(export.name, kind, export.index);
                }
                exports.export("kb_meter", ExportKind::Global, index);
                result.section(&exports);
            }
            Payload::StartSection { .. } => {
                return Err("the embedded module cannot have a start function".into());
            }
            Payload::CodeSectionStart { .. } => code = Some(CodeSection::new()),
            Payload::CodeSectionEntry(body) => {
                let index = global_index.ok_or("missing embedded module exports")?;
                code.as_mut()
                    .unwrap()
                    .raw(&meter_function(bytes, body, index)?);
            }
            Payload::CustomSection(_) => {}
            other => {
                if let Some((id, range)) = other.as_section() {
                    if let Some(code) = code.take() {
                        result.section(&code);
                    }
                    result.section(&RawSection {
                        id,
                        data: &bytes[range],
                    });
                }
            }
        }
    }
    if let Some(code) = code {
        result.section(&code);
    }
    if memory_count != 1 {
        return Err("embedded module has no linear memory".into());
    }
    let result = result.finish();
    Validator::new()
        .validate_all(&result)
        .map_err(|error| error.to_string())?;
    Ok(result)
}

fn meter_function(bytes: &[u8], body: FunctionBody<'_>, fuel: u32) -> Result<Vec<u8>, String> {
    let mut reader = body
        .get_operators_reader()
        .map_err(|error| error.to_string())?;
    let mut checkpoint = vec![];
    for instruction in [
        Instruction::GlobalGet(fuel),
        Instruction::I32Const(16),
        Instruction::I32LtU,
        Instruction::If(wasm_encoder::BlockType::Empty),
        Instruction::Unreachable,
        Instruction::End,
        Instruction::GlobalGet(fuel),
        Instruction::I32Const(16),
        Instruction::I32Sub,
        Instruction::GlobalSet(fuel),
    ] {
        instruction.encode(&mut checkpoint);
    }
    let range = body.range();
    let mut body = Vec::with_capacity(range.len());
    let mut previous = reader.original_position();
    body.extend_from_slice(&bytes[range.start..previous]);
    body.extend_from_slice(&checkpoint);
    let mut count = 0;
    while !reader.eof() {
        let op = reader.read().map_err(|error| error.to_string())?;
        count += 1;
        if matches!(op, Operator::Loop { .. }) || count == 32 && !reader.eof() {
            let next = reader.original_position();
            body.extend_from_slice(&bytes[previous..next]);
            body.extend_from_slice(&checkpoint);
            previous = next;
            count = 0;
        }
    }
    body.extend_from_slice(&bytes[previous..range.end]);
    Ok(body)
}

pub fn compile(source: &Path, output: &Path, platform: &Path, keyboard: bool) {
    let binary = env::var_os("WASM2C").unwrap_or_else(|| "wasm2c".into());
    println!("cargo:rerun-if-env-changed=WASM2C");
    println!("cargo:rerun-if-changed={}", source.display());
    println!("cargo:rerun-if-changed={}", platform.display());
    let version = Command::new(&binary)
        .arg("--version")
        .output()
        .expect("WABT 1.0.42 wasm2c is required to build embedded WebAssembly");
    assert!(
        version.status.success() && String::from_utf8_lossy(&version.stdout).trim() == "1.0.42",
        "WABT 1.0.42 is required to build embedded WebAssembly"
    );
    let bytes = fs::read(source).unwrap();
    let bytes =
        inject_fuel(&bytes, keyboard).expect("cannot meter the embedded WebAssembly module");
    let metered = output.join("kb-guest.wasm");
    fs::write(&metered, bytes).unwrap();
    let translated = output.join("kb_guest.c");
    let result = Command::new(binary)
        .arg(&metered)
        .args([
            "--no-debug-names",
            "--disable-exceptions",
            "--disable-simd",
            "--module-name",
            "kb_guest",
            "-o",
        ])
        .arg(&translated)
        .status()
        .unwrap();
    assert!(
        result.success(),
        "wasm2c failed to translate the embedded component"
    );

    let text = fs::read_to_string(&translated).unwrap();
    let text =
        normalize_tail_declarations(&text).expect("cannot normalize wasm2c local declarations");
    let text = text.replace(
        "void *instance_ptr_storage;",
        "void *instance_ptr_storage = instance;",
    );
    fs::write(&translated, text).unwrap();

    let target = env::var("TARGET").unwrap();
    let target_key = target.replace('-', "_");
    println!("cargo:rerun-if-env-changed=CC_{target_key}");
    println!("cargo:rerun-if-env-changed=CLANG_PATH");
    let mut build = cc::Build::new();
    let compiler = env::var_os(format!("CC_{target_key}"))
        .or_else(|| env::var_os("CC"))
        .or_else(|| env::var_os("CLANG_PATH"))
        .unwrap_or_else(|| "clang".into());
    build
        .compiler(compiler)
        .std("c99")
        .flag("-ffreestanding")
        .flag("-Werror=frame-larger-than")
        .flag("-Wno-unused-parameter")
        .flag("-Wno-unused-function")
        .flag("-Wno-unused-variable")
        .flag("-Wno-unused-label")
        .flag("-Wno-unused-but-set-variable")
        .flag("-Wno-sign-compare")
        .flag("-Wframe-larger-than=2048")
        .opt_level_str("s")
        .warnings(true)
        .include(platform)
        .include(platform.join("freestanding"))
        .include(output)
        .define("KB_EMBEDDED_FREESTANDING", None)
        .define("WASM_RT_USE_MMAP", "0")
        .define("WASM_RT_MEMCHECK_BOUNDS_CHECK", "1")
        .define("WASM_RT_STACK_DEPTH_COUNT", "1")
        .define(
            "WASM_RT_MAX_CALL_STACK_DEPTH",
            "(kb_stack_available() ? 64 : 0)",
        )
        .flag("-include")
        .flag(platform.join("guard.h").to_str().unwrap())
        .file(translated)
        .file(platform.join("bridge.c"))
        .file(platform.join("jump.S"));
    if keyboard {
        build.define("KB_EMBEDDED_KEYBOARD", None);
    }
    let llvm_ar = rust_archiver();
    if let Some(archiver) = llvm_ar {
        build.archiver(archiver);
        build.ar_flag("--format=gnu");
    }
    build.compile("kb_embedded_guest");
}

fn rust_archiver() -> Option<PathBuf> {
    if !env::var("HOST").ok()?.ends_with("-apple-darwin") || env::var_os("AR").is_some() {
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

fn normalize_tail_declarations(text: &str) -> Result<String, String> {
    let mut output = String::with_capacity(text.len());
    let mut pending_type = None;
    for line in text.lines() {
        let mut line = line;
        if let Some(after) = line.strip_prefix("  ") {
            if let Some((kind, values)) = after.split_once("   ") {
                if matches!(kind, "u32" | "u64" | "f32" | "f64" | "v128")
                    && values.contains(" var_")
                {
                    if pending_type.replace(kind).is_some() {
                        return Err("unexpected wasm2c local declaration structure".into());
                    }
                    output.push_str("  ");
                    line = values;
                }
            }
        } else if line.starts_with("var_l") {
            let kind = pending_type.take().ok_or("missing wasm2c local type")?;
            output.push_str("  ");
            output.push_str(kind);
            output.push(' ');
        }
        output.push_str(line);
        output.push('\n');
    }
    if pending_type.is_some() {
        return Err("incomplete wasm2c local declaration".into());
    }
    Ok(output)
}
