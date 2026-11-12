# kb.wlt

Rust keyboard firmware whose behavior comes from composable [Wavelet](https://github.com/logaan/wavelet) WebAssembly Components. WIT interfaces thread state explicitly through input and output middleware. The first target is the nRF52840.

Development builds cover XIAO LED control, Paintbrush USB/Bluetooth braille, split keyboards, Prospector receivers, and Toucan 2 with optional displays. Hardware operation is unverified. Start with the build scripts and hardware checklists in [milestones](milestones/), or the separate [two-board reset controller](resetter/).

```sh
rustup target add thumbv7em-none-eabihf
cargo test --workspace
cargo xtask build xiao-led
cargo xtask build paintbrush-usb
```

Building components requires the current Wavelet CLI on `PATH`; builds check component interfaces, sample runtime heap use, and reserve native stack RAM before producing firmware. Wasmi interprets validated core modules extracted from the components, with explicit canonical ABI adapters. The optional two-component Toucan example also requires Binaryen and QEMU, and supports firmware without displays. Tests use checked-in component artifacts. CI runs on pushes and pull requests to `master`; the manual Release workflow bumps the version and publishes tagged firmware bundles.
