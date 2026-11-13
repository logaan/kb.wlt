# Milestone 1: XIAO nRF52840 Sense Plus LED

The firmware cross-compiles and the Wavelet component passes host tests. The checklist below is for your hardware confirmation; no hardware tests have been performed.

- [x] Install the Rust target with `rustup target add thumbv7em-none-eabihf`.
- [x] Confirm `wavelet` is on PATH, or set `WAVELET=/absolute/path/to/wavelet`.
- [x] Run `./milestones/01-xiao-led/build.sh` from the repository root. It compiles this folder's `led.wlt`, validates its component interface, and produces `dist/xiao-led.uf2`.
- [x] Connect the XIAO Sense Plus with a USB data cable and double-press RESET to enter its UF2 bootloader.
- [x] Read `INFO_UF2.TXT` on the mounted volume and confirm it is the XIAO. This image starts at `0x27000`, for its stock S140 v7 bootloader. A `nosd` bootloader requires `build.sh --nosd` and the resulting `dist/xiao-led-nosd.uf2` instead.
- [x] Run `./milestones/01-xiao-led/flash.sh /Volumes/XIAO-SENSE` with the actual mounted volume path. For a nondefault image, use `./scripts/flash-uf2.sh dist/xiao-led-nosd.uf2 /Volumes/ACTUAL_VOLUME`.
- [x] Confirm the RGB LED cycles red → yellow → green → cyan → blue → magenta, about one second per color.
- [x] Disconnect and reconnect USB; confirm the cycle restarts.
- [wontdo] Change the `1000` period in `led.wlt` to `250`, rebuild, flash, and confirm the cycle is faster. This verifies that the behavior comes from your Wavelet component.
- [wontdo] Restore the period if desired.
- [x] Run `./milestones/01-xiao-led/build-composed.sh` and flash `dist/xiao-led-composed.uf2` with `./scripts/flash-uf2.sh dist/xiao-led-composed.uf2 /Volumes/ACTUAL_VOLUME`.
- [x] Confirm the same cycle appears dimmer. This build composes `led-dim.wlt` around `led.wlt`, with both components running on the controller.
- [wontdo] Change the brightness divisor in `led-dim.wlt`, rebuild and flash the composed image, and confirm the brightness changes.

There is no button-speed requirement. The built-in button remains RESET and retains its bootloader role.
