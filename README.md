# kb.wlt

A keyboard firmware monorepo: Rust hosts run composable [Wavelet](https://github.com/logaan/wavelet) WebAssembly Components on nRF52840 controllers.

| Directory | Responsibility |
| --- | --- |
| [toucan/](toucan/) | Toucan firmware, matrix protocol, display layouts, and board tests |
| [paintbrush/](paintbrush/) | Paintbrush USB/Bluetooth firmware and split receiver applications |
| [host/](host/) | Rust Wasm component runtime, adapters, and Cortex-M heap probe |
| [clients/](clients/) | All Wavelet clients, WIT contracts, component build tooling, and generated Wasm |
| [shared/](shared/) | Keyboard primitives, display controllers, and persistent storage |
| [firmware/](firmware/) | Shared nRF52840 platform, radio, USB, and firmware build manifest |
| [resetter/](resetter/) | Independent two-board reset controller |
| `xtask/`, `scripts/` | Target catalog, build/flash tools, and release automation |

## Build and flash

Run commands from the repository root. Install Rust, the current Wavelet CLI on `PATH`, and Clang/libclang for the Nordic bindings. Binaryen (`wasm-opt`) and QEMU (`qemu-system-arm`) are also required for composed-keyboard builds and full release verification.

```sh
rustup target add thumbv7em-none-eabihf
cargo test --workspace --locked
cargo xtask targets
cargo xtask build toucan-left-satellite-display
cargo xtask build toucan-right-satellite
cargo xtask build toucan-prospector
```

Images are written to `dist/`. Enter the appropriate board's UF2 bootloader and flash one board at a time, using its actual mounted volume:

```sh
./scripts/flash-uf2.sh dist/toucan-prospector.uf2 /Volumes/XIAO-SENSE
```

See the board READMEs for other configurations and bootloader variants. Normal builds compile the canonical client source; release builds use its checked-in artifacts. After editing client sources, run `./scripts/build-components.sh` and commit the generated changes as well.

## Validation and releases

The upgraded Toucan 1 with a left MIP display, two battery-powered halves, and a USB Prospector receiver was accepted in hardware testing. Its two-layer keymap is now the default; braille bindings remain inactive. Other configurations are not implied to be hardware-validated, and this repository reorganization still needs future hardware regression testing.

`cargo xtask check` runs workspace tests and shared-crate cross-checks. `./scripts/build-artifacts.sh` cross-builds all firmware targets, runs component heap probes, and writes release UF2s to `dist/release/`. Neither command flashes hardware.

Push development commits to `master`. Publish versions through the manual **Release** GitHub Actions workflow, which synchronizes versions using `scripts/release-version.py`, runs tests/builds, creates a release commit and tag, and publishes UF2s, a source bundle, and checksums. Do not create release tags manually.
