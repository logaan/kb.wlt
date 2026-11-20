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
| `xtask/`, `scripts/` | Target catalog, build/flash tools, and release automation |

## Build and flash

Run commands from the repository root. Install Rust, the current Wavelet CLI, Binaryen (`wasm-opt`), [WABT 1.0.42](https://github.com/WebAssembly/wabt/releases/tag/1.0.42) (`wasm2c`), and Clang with libclang and ARM support.

```sh
rustup target add thumbv7em-none-eabihf
cargo xtask check
cargo xtask targets
cargo xtask build toucan-left-satellite-display
cargo xtask build toucan-right-satellite
cargo xtask build toucan-prospector
```

Images are written to `dist/`. Enter the appropriate board's UF2 bootloader and flash one board at a time, using its actual mounted volume:

```sh
./scripts/flash-uf2.sh dist/toucan-prospector.uf2 /Volumes/XIAO-SENSE
```

A release contains six UF2s across the Toucan and Paintbrush install kits. Use the matching images from the same kit. The Toucan left half handles typing and can send reports directly over Bluetooth or through its USB Prospector. Paintbrush sends reports through its USB Prospector. Normal builds compile the canonical client source. Release builds use its checked-in artifacts. After editing client sources, run `./scripts/build-components.sh` and commit the generated changes.

## Validation and releases

The [browser emulator](emulator/) runs the Toucan component, both displays, and both simulated transports. The earlier Toucan USB configuration passed hardware testing. The current firmware and the Paintbrush kit still need hardware validation.

`cargo xtask check` runs workspace tests and shared-crate cross-checks. `./scripts/build-artifacts.sh` cross-builds the six supported images, runs component heap probes, and replaces the UF2s in `dist/release/`. `cargo xtask verify-release dist/release` rejects missing or extra images. Neither command flashes hardware.

Push development commits to `master`. Publish versions through the manual **Release** GitHub Actions workflow, which synchronizes versions using `scripts/release-version.py`, runs tests/builds, creates a release commit and tag, and publishes one install ZIP per keyboard plus checksums. Each ZIP contains its left, right, and receiver UF2s, `INSTALL.txt`, the flash helper, and UTF-8 `KEYMAP.txt` and `BRAILLE.txt` references. Do not create release tags manually.
