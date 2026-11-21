# Scripts

- [build-artifacts.sh](build-artifacts.sh) builds the six production UF2 images and checksums from checked-in Wasm for CI and releases.
- [build-components.sh](build-components.sh) regenerates the checked-in Wasm for firmware, emulator, and tests after Wavelet changes.
- [ci-clang.sh](ci-clang.sh) sets up Clang, libclang, and WABT on GitHub Actions runners.
- [clean.sh](clean.sh) removes local build outputs and stale artifacts.
- [flash-uf2.sh](flash-uf2.sh) flashes a UF2 to a mounted bootloader volume using supplied paths or interactive menus.
- [package-release.py](package-release.py) packages the six release images into two install kits with instructions and checksums.
- [release-version.py](release-version.py) synchronizes release versions in the Cargo manifests and prepares changelog notes.
- [test-embedded.sh](test-embedded.sh) builds and runs the Braille and keyboard Cortex-M runtime probes in QEMU.
- [tests/test_release_version.py](tests/test_release_version.py) tests version validation and changelog handling.
