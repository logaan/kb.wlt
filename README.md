# kb.wlt

Rust keyboard firmware whose behavior comes from composable [Wavelet](https://github.com/logaan/wavelet) WebAssembly Components. WIT interfaces thread state explicitly through input and output middleware. The first target is the nRF52840.

Development builds cover XIAO LED control and Paintbrush USB/Bluetooth braille, with display support. Hardware validation is ongoing. Start with the build scripts and hardware checks in [milestones](milestones/), or the separate [two-board reset controller](resetter/).

```sh
rustup target add thumbv7em-none-eabihf
cargo test --workspace
cargo xtask build xiao-led
cargo xtask build paintbrush-usb
```

Building components requires the Wavelet CLI on `PATH`. Tests use checked-in component artifacts. CI runs on pushes and pull requests to `master`; the manual Release workflow bumps the version and publishes tagged firmware bundles.
