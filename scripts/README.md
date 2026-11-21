# Scripts

- [build](build) regenerates Wasm and builds the six flashable UF2s and checksums in `dist/release/`, or just regenerates Wasm with `--wasm-only`.
- [test](test) runs workspace tests, cross-checks, and release-script tests, or runs the QEMU probes with `--embedded`.
- [flash](flash) flashes a UF2 to a mounted bootloader volume using supplied paths or interactive menus.
- [release](release) dispatches the GitHub Release workflow on `master` with `patch`, `minor`, `major`, or `set VERSION`.
- [clean](clean) removes local build outputs and stale artifacts.

## Helpers

- [ci/build-wasm](ci/build-wasm) regenerates the checked-in Wasm for firmware, the emulator, and tests.
- [ci/build-uf2](ci/build-uf2) builds the six production images and checksums from generated Wasm.
- [ci/ci-clang.sh](ci/ci-clang.sh) sets up Clang, libclang, and WABT on GitHub Actions runners.
- [ci/package-release.py](ci/package-release.py) packages the six release images into two install kits with instructions and checksums.
- [ci/release-version.py](ci/release-version.py) synchronizes the Cargo manifest versions and prepares release notes.
- [ci/test-embedded.sh](ci/test-embedded.sh) builds and runs the Braille and keyboard Cortex-M runtime probes in QEMU.
- [ci/tests/test_release_version.py](ci/tests/test_release_version.py) tests version validation and changelog handling.
