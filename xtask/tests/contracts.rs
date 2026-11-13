#[test]
fn firmware_contracts_are_valid_and_resource_free() {
    let mut resolve = wit_parser::Resolve::default();
    let (package, _) = resolve
        .push_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../clients/wit"))
        .unwrap();
    assert!(
        resolve.packages[package]
            .worlds
            .contains_key("keyboard-module")
    );
    assert!(resolve.types.iter().all(|(_, ty)| !matches!(
        ty.kind,
        wit_parser::TypeDefKind::Resource | wit_parser::TypeDefKind::Handle(_)
    )));
}
