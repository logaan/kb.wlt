# kb.wlt

A keyboard firmware monorepo: Rust hosts run composable [Wavelet](https://github.com/logaan/wavelet) WebAssembly Components on nRF52840 controllers.

| Directory | Responsibility |
| --- | --- |
| [toucan/](toucan/) | Toucan firmware, matrix protocol, display layouts, and board tests |
| [paintbrush/](paintbrush/) | Paintbrush satellites and their Prospector receiver |
| [host/](host/) | Rust Wasm component runtime, adapters, and resource checks |
| [clients/](clients/) | All Wavelet clients, WIT contracts, component build tooling, and generated Wasm |
| [shared/](shared/) | Keyboard primitives, display controllers, and persistent storage |
| [firmware/](firmware/) | Shared nRF52840 platform, radio, USB, and firmware build manifest |
| `xtask/`, `scripts/` | Target catalog, build/flash tools, and release automation |

## Build and flash

Run commands from the repository root. Install Rust, the current Wavelet CLI on `PATH`, and Clang/libclang for the Nordic bindings. Binaryen (`wasm-opt`) is needed only when regenerating the host's optimized middleware test fixtures.

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

Only two configurations are supported: Toucan left/right satellites with a Prospector receiver, and Paintbrush left/right satellites with a Prospector receiver. Releases contain exactly those six UF2s. See the board READMEs for their build commands and bootloader variants. Normal builds compile the canonical client source; release builds use its checked-in artifacts. After editing client sources, run `./scripts/build-components.sh` and commit the generated changes as well.

## Validation and releases

The upgraded Toucan 1 with a left MIP display, two battery-powered halves, and a USB Prospector receiver was accepted in hardware testing. Its two-layer keymap is now the default; braille bindings remain inactive. The Paintbrush setup and subsequent implementation cleanup still need future hardware regression testing.

`cargo xtask check` runs workspace tests and shared-crate cross-checks. `./scripts/build-artifacts.sh` cross-builds the six supported images, runs component heap probes, and replaces the UF2s in `dist/release/`. `cargo xtask verify-release dist/release` rejects missing or extra images. Neither command flashes hardware.

Push development commits to `master`. Publish versions through the manual **Release** GitHub Actions workflow, which synchronizes versions using `scripts/release-version.py`, runs tests/builds, creates a release commit and tag, and publishes UF2s, a source bundle, and checksums. Do not create release tags manually.
