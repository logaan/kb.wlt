use crate::Contract;
use std::collections::BTreeMap;
use wasmparser::{
    CanonicalFunction, CanonicalOption, ComponentAlias, ComponentExternalKind, ComponentInstance,
    ComponentTypeRef, ExternalKind, Instance, Parser, Payload,
};

#[derive(Clone, Debug)]
enum Function {
    Imported(String, String),
    Lifted {
        module: u32,
        function: String,
        memory: Option<(u32, String)>,
        post: Option<(u32, String)>,
    },
}

#[derive(Clone, Debug)]
enum CoreFunction {
    Export(u32, String),
    Lowered,
}

#[derive(Clone)]
enum CoreInstance {
    Module(u32),
    Exports(BTreeMap<String, CoreFunction>),
}

type Exports = BTreeMap<String, Function>;

pub(crate) fn validate(component: &[u8], contract: Contract) -> Result<(), String> {
    let mut depth = 0;
    let mut shims = Vec::new();
    let mut instances: Vec<Exports> = Vec::new();
    let mut functions: Vec<Function> = Vec::new();
    let mut core_instances: Vec<CoreInstance> = Vec::new();
    let mut core_functions: Vec<CoreFunction> = Vec::new();
    let mut memories = Vec::new();
    let mut public = None;
    for payload in Parser::new(0).parse_all(component) {
        let payload = payload.map_err(|e| e.to_string())?;
        if depth > 0 {
            match payload {
                Payload::ModuleSection { .. } | Payload::ComponentSection { .. } => depth += 1,
                Payload::End(_) => depth -= 1,
                _ => {}
            }
            continue;
        }
        match payload {
            Payload::ModuleSection { .. } => depth += 1,
            Payload::ComponentSection {
                unchecked_range, ..
            } => {
                shims.push(shim(&component[unchecked_range])?);
                depth += 1;
            }
            Payload::ComponentImportSection(section) => {
                for import in section {
                    let import = import.map_err(|e| e.to_string())?;
                    match import.ty {
                        ComponentTypeRef::Instance(_)
                            if import.name.name == contract.interface() =>
                        {
                            instances.push(
                                contract
                                    .functions()
                                    .iter()
                                    .map(|name| {
                                        (
                                            name.to_string(),
                                            Function::Imported(
                                                import.name.name.into(),
                                                name.to_string(),
                                            ),
                                        )
                                    })
                                    .collect(),
                            );
                        }
                        _ => return Err("unsupported top-level component import".into()),
                    }
                }
            }
            Payload::InstanceSection(section) => {
                for instance in section {
                    core_instances.push(match instance.map_err(|e| e.to_string())? {
                        Instance::Instantiate { module_index, .. } => {
                            CoreInstance::Module(module_index)
                        }
                        Instance::FromExports(exports) => CoreInstance::Exports(
                            exports
                                .iter()
                                .filter(|export| export.kind == ExternalKind::Func)
                                .map(|export| {
                                    Ok((
                                        export.name.to_owned(),
                                        lookup(&core_functions, export.index)?.clone(),
                                    ))
                                })
                                .collect::<Result<_, String>>()?,
                        ),
                    });
                }
            }
            Payload::ComponentAliasSection(section) => {
                for alias in section {
                    match alias.map_err(|e| e.to_string())? {
                        ComponentAlias::CoreInstanceExport {
                            kind: ExternalKind::Func,
                            instance_index,
                            name,
                        } => {
                            let function = match lookup(&core_instances, instance_index)? {
                                CoreInstance::Module(module) => {
                                    CoreFunction::Export(*module, name.into())
                                }
                                CoreInstance::Exports(exports) => exports
                                    .get(name)
                                    .ok_or("missing core function alias")?
                                    .clone(),
                            };
                            core_functions.push(function);
                        }
                        ComponentAlias::CoreInstanceExport {
                            kind: ExternalKind::Memory,
                            instance_index,
                            name,
                        } => {
                            let CoreInstance::Module(module) =
                                lookup(&core_instances, instance_index)?
                            else {
                                return Err("unsupported memory alias".into());
                            };
                            memories.push((*module, name.to_owned()));
                        }
                        ComponentAlias::CoreInstanceExport {
                            kind: ExternalKind::Table,
                            ..
                        } => {}
                        ComponentAlias::InstanceExport {
                            kind: ComponentExternalKind::Func,
                            instance_index,
                            name,
                        } => {
                            functions.push(
                                lookup(&instances, instance_index)?
                                    .get(name)
                                    .ok_or("missing component function alias")?
                                    .clone(),
                            );
                        }
                        ComponentAlias::InstanceExport {
                            kind: ComponentExternalKind::Type,
                            ..
                        } => {}
                        _ => return Err("unsupported component alias".into()),
                    }
                }
            }
            Payload::ComponentCanonicalSection(section) => {
                for canonical in section {
                    match canonical.map_err(|e| e.to_string())? {
                        CanonicalFunction::Lift {
                            core_func_index,
                            options,
                            ..
                        } => {
                            let CoreFunction::Export(module, name) =
                                lookup(&core_functions, core_func_index)?
                            else {
                                return Err("cannot lift an imported core function".into());
                            };
                            let mut memory = None;
                            let mut post = None;
                            for option in options {
                                match option {
                                    CanonicalOption::Memory(index) => {
                                        memory = Some(lookup(&memories, index)?.clone())
                                    }
                                    CanonicalOption::PostReturn(index) => {
                                        let CoreFunction::Export(module, name) =
                                            lookup(&core_functions, index)?
                                        else {
                                            return Err("invalid canonical post-return".into());
                                        };
                                        post = Some((*module, name.clone()));
                                    }
                                    CanonicalOption::Realloc(index) => {
                                        if !matches!(lookup(&core_functions, index)?, CoreFunction::Export(0, name) if name == "cabi_realloc")
                                        {
                                            return Err(
                                                "invalid canonical reallocation function".into()
                                            );
                                        }
                                    }
                                    CanonicalOption::UTF8 => {}
                                    _ => return Err("unsupported canonical lift option".into()),
                                }
                            }
                            functions.push(Function::Lifted {
                                module: *module,
                                function: name.clone(),
                                memory,
                                post,
                            });
                        }
                        CanonicalFunction::Lower {
                            func_index,
                            options,
                        } => {
                            let Function::Imported(interface, name) =
                                lookup(&functions, func_index)?
                            else {
                                return Err("unsupported canonical lower target".into());
                            };
                            if interface != contract.interface()
                                || !contract.functions().contains(&name.as_str())
                            {
                                return Err("unexpected downstream function".into());
                            }
                            for option in options {
                                match option {
                                    CanonicalOption::Memory(index)
                                        if lookup(&memories, index)? == &(0, "memory".into()) => {}
                                    CanonicalOption::Realloc(index) if matches!(lookup(&core_functions, index)?, CoreFunction::Export(0, name) if name == "cabi_realloc") =>
                                        {}
                                    CanonicalOption::UTF8 => {}
                                    _ => return Err("unsupported canonical lower option".into()),
                                }
                            }
                            core_functions.push(CoreFunction::Lowered);
                        }
                        _ => return Err("unsupported canonical operation".into()),
                    }
                }
            }
            Payload::ComponentInstanceSection(section) => {
                for instance in section {
                    let exports = match instance.map_err(|e| e.to_string())? {
                        ComponentInstance::Instantiate {
                            component_index,
                            args,
                        } => lookup(&shims, component_index)?
                            .iter()
                            .map(|(name, import)| {
                                let arg = args
                                    .iter()
                                    .find(|arg| {
                                        arg.kind == ComponentExternalKind::Func
                                            && arg.name == import
                                    })
                                    .ok_or("missing shim function argument")?;
                                Ok((name.clone(), lookup(&functions, arg.index)?.clone()))
                            })
                            .collect::<Result<_, String>>()?,
                        ComponentInstance::FromExports(exports) => exports
                            .iter()
                            .filter(|export| export.kind == ComponentExternalKind::Func)
                            .map(|export| {
                                Ok((
                                    export.name.name.into(),
                                    lookup(&functions, export.index)?.clone(),
                                ))
                            })
                            .collect::<Result<_, String>>()?,
                    };
                    instances.push(exports);
                }
            }
            Payload::ComponentExportSection(section) => {
                for export in section {
                    let export = export.map_err(|e| e.to_string())?;
                    match export.kind {
                        ComponentExternalKind::Instance => {
                            let instance = lookup(&instances, export.index)?.clone();
                            if export.name.name == contract.interface() {
                                public = Some(instance.clone());
                            }
                            instances.push(instance);
                        }
                        ComponentExternalKind::Type => {}
                        _ => return Err("unsupported top-level component export".into()),
                    }
                }
            }
            Payload::Version { .. }
            | Payload::ComponentTypeSection(_)
            | Payload::CustomSection(_)
            | Payload::End(_) => {}
            _ => return Err("unsupported component execution section".into()),
        }
    }
    let public = public.ok_or("missing canonical interface export")?;
    for name in contract.functions() {
        let Some(Function::Lifted {
            module: 0,
            function,
            memory,
            post,
        }) = public.get(*name)
        else {
            return Err(format!("{name} is not lifted from the embedded module"));
        };
        let expected = format!("{}#{name}", contract.interface());
        if function != &expected || post != &Some((0, format!("cabi_post_{expected}"))) {
            return Err(format!(
                "{name} has incompatible canonical function bindings"
            ));
        }
        if *name != "map-cell" && memory != &Some((0, "memory".into())) {
            return Err(format!("{name} has an incompatible canonical memory"));
        }
    }
    Ok(())
}

fn lookup<T>(items: &[T], index: u32) -> Result<&T, String> {
    items
        .get(index as usize)
        .ok_or_else(|| "invalid component binding index".into())
}

fn shim(bytes: &[u8]) -> Result<BTreeMap<String, String>, String> {
    let mut imports = Vec::new();
    let mut exports = BTreeMap::new();
    for payload in Parser::new(0).parse_all(bytes) {
        match payload.map_err(|e| e.to_string())? {
            Payload::ComponentImportSection(section) => {
                for import in section {
                    let import = import.map_err(|e| e.to_string())?;
                    match import.ty {
                        ComponentTypeRef::Func(_) => imports.push(import.name.name.to_owned()),
                        ComponentTypeRef::Type(_) => {}
                        _ => return Err("unsupported interface shim import".into()),
                    }
                }
            }
            Payload::ComponentExportSection(section) => {
                for export in section {
                    let export = export.map_err(|e| e.to_string())?;
                    match export.kind {
                        ComponentExternalKind::Func => {
                            let import = lookup(&imports, export.index)?.clone();
                            exports.insert(export.name.name.into(), import.clone());
                            imports.push(import);
                        }
                        ComponentExternalKind::Type => {}
                        _ => return Err("unsupported interface shim export".into()),
                    }
                }
            }
            Payload::Version { .. }
            | Payload::ComponentTypeSection(_)
            | Payload::CustomSection(_)
            | Payload::End(_) => {}
            _ => return Err("embedded interface shim contains executable code".into()),
        }
    }
    Ok(exports)
}
