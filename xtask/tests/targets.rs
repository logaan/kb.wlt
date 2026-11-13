use xtask::targets::{TARGETS, find, verify_release_names};

const EXPECTED: [&str; 6] = [
    "toucan-left-satellite-display",
    "toucan-right-satellite",
    "toucan-prospector",
    "paintbrush-left-satellite",
    "paintbrush-right-satellite",
    "prospector-pair",
];

#[test]
fn supported_targets_are_exactly_two_three_board_setups() {
    assert_eq!(
        TARGETS.iter().map(|target| target.name).collect::<Vec<_>>(),
        EXPECTED
    );
    for name in [
        "xiao-led",
        "paintbrush-usb",
        "paintbrush-ble",
        "toucan-ble",
        "prospector-single",
        "prospector-display-test",
        "settings-reset-xiao",
        "resetter",
        "toucan-dongle-composed",
    ] {
        assert!(find(name).is_none(), "{name}");
    }
    for target in TARGETS {
        assert!(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join(target.source())
                .is_file()
        );
    }
}

#[test]
fn releases_reject_missing_and_stale_firmware() {
    let names = EXPECTED
        .iter()
        .map(|name| format!("{name}.uf2"))
        .collect::<Vec<_>>();
    assert!(verify_release_names(&names).is_ok());
    for index in 0..names.len() {
        let mut missing = names.clone();
        missing.remove(index);
        assert!(verify_release_names(&missing).is_err());
    }
    let mut stale = names.clone();
    stale.push("prospector-display-test.uf2".into());
    assert!(verify_release_names(&stale).is_err());
    let mut packaged = names;
    packaged.extend(["SHA256SUMS".into(), "kb.wlt-0.1.1.tar.gz".into()]);
    assert!(verify_release_names(&packaged).is_ok());
}
