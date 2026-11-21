# kb.wlt

A keyboard firmware monorepo: Rust hosts run [Wavelet](https://github.com/logaan/wavelet) WebAssembly Components on nRF52840 controllers.

| Directory | Responsibility |
| --- | --- |
| [toucan/](toucan/) | Toucan firmware, matrix protocol, display layouts, and board tests |
| [paintbrush/](paintbrush/) | Paintbrush satellites and their Prospector receiver |
| [host/](host/) | Rust Wasm component runtime, adapters, and resource checks |
| [clients/](clients/) | All Wavelet clients, WIT contracts, component build tooling, and generated Wasm |
| [shared/](shared/) | Keyboard primitives, display controllers, and persistent storage |
| [firmware/](firmware/) | Shared nRF52840 platform, radio, USB, and firmware build manifest |
| [emulator/](emulator/) | Browser simulator for the Toucan and Prospector |
| `xtask/`, [scripts/](scripts/) | Target catalog, build/flash tools, and release automation |

## Build and flash

Run commands from the repository root. Install Rust, the current Wavelet CLI, Binaryen (`wasm-opt`), [WABT 1.0.42](https://github.com/WebAssembly/wabt/releases/tag/1.0.42) (`wasm2c`), and Clang with libclang and ARM support.

```sh
rustup target add thumbv7em-none-eabihf
./scripts/test
./scripts/build
```

The build regenerates Wasm and writes all six UF2 images to `dist/release/`. For a single board, run `cargo xtask targets` and `cargo xtask build TARGET` to write its image to `dist/`. Enter the board's UF2 bootloader and use the flash menu or pass both paths:

```sh
./scripts/flash
./scripts/flash dist/release/toucan-prospector.uf2 /Volumes/XIAO-SENSE
```

A release contains six UF2s across the Toucan and Paintbrush install kits. Use the matching images from the same kit. The Toucan left half handles typing and can send reports through Prospector, to one of five Bluetooth hosts, or over its own USB port. Paintbrush sends reports through its USB Prospector. After editing Wavelet sources, run `./scripts/build` or `./scripts/build --wasm-only` and commit the generated changes. Release builds use the checked-in Wasm.

## Validation and releases

The [browser emulator](emulator/) runs the Toucan component, both displays, and simulated host targets and touch input. The earlier Toucan USB configuration passed hardware testing. The current firmware and the Paintbrush kit still need hardware validation.

`./scripts/test` runs workspace tests, shared-crate cross-checks, and release-script tests. `./scripts/test --embedded` runs the Cortex-M probes in QEMU. `./scripts/clean` removes local build outputs.

Push development commits to `master`. With GitHub CLI authenticated, run `./scripts/release patch` to dispatch the **Release** workflow, or start it from GitHub Actions. The workflow synchronizes versions, runs tests and builds, creates a release commit and tag, and publishes one install ZIP per keyboard plus checksums. Each ZIP contains its left, right, and receiver UF2s, `INSTALL.txt`, the flash helper, and UTF-8 `KEYMAP.txt` and `BRAILLE.txt` references.
