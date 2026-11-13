# Toucan

Firmware and board-specific tests for the upgraded Toucan 1, using two XIAO nRF52840 controllers and the retained 144×168 MIP display. The trackpad is not implemented. Internal `nice_view` names refer to this MIP panel, not a 160×68 nice!view display.

`firmware/` contains the matrix protocol, Bluetooth application, MIP driver, and binary entry points. `display/` contains the Toucan MIP and Prospector dashboard layouts. The shared Cargo manifests explicitly include these modules and tests; they are not separate copies of the platform or host.

All Wavelet code lives in [clients/](../clients/). The canonical keymap is [toucan-keymap.wlt](../clients/components/toucan-keymap/toucan-keymap.wlt). The firmware target catalog and release artifacts use that same layout.

## Normal configuration: two halves and Prospector

From the repository root:

```sh
cargo xtask build toucan-left-satellite-display
cargo xtask build toucan-right-satellite
cargo xtask build toucan-prospector
```

Flash one board at a time, entering its bootloader first and substituting its actual volume path:

```sh
./scripts/flash-uf2.sh dist/toucan-left-satellite-display.uf2 /Volumes/XIAO-BOOT
./scripts/flash-uf2.sh dist/toucan-right-satellite.uf2 /Volumes/XIAO-BOOT
./scripts/flash-uf2.sh dist/toucan-prospector.uf2 /Volumes/XIAO-SENSE
```

Keep Prospector connected by USB; power the halves from batteries or USB. The halves connect automatically to Prospector, not to the computer's Bluetooth settings. The host sees **kb.wlt Toucan Receiver**. The receiver executes the keymap and sends status to the left MIP display.

For keymap-only changes, rebuild and reflash only `toucan-prospector`. Regenerate checked-in client artifacts with `./scripts/build-components.sh` before committing or releasing. A one-off source can be selected with `cargo xtask build toucan-prospector --source path/to/keymap.wlt`.

The default application origin is stock XIAO S140 v7 at `0x27000`. Builds also accept `--s140v6` or `--nosd` for boards with matching bootloaders; flash the corresponding suffixed image. Do not select an origin without checking the board's bootloader.

## Current behavior and limits

The default is the tested two-layer conventional layout. Hold the left outer thumb to access navigation and modifier bindings; release it to return to the base layer. Braille dot keys, Braille Space, mode selection, and settings placeholders are inactive. There is no Toucan braille engine or tap-to-latch behavior yet.

Display layer reporting still follows the original rightmost-thumb convention rather than introspecting the component's layer state. The Prospector dashboard is 280×240 landscape with an inset title for rounded corners. Its backlight is currently fully on.

The two-half/Prospector configuration was accepted by the user after typing, display, cross-half modifier, reconnection, and power-cycle checks. Subsequent code cleanup still needs hardware regression testing. Direct Bluetooth HID, display-free left halves, and composed-firmware variants are not supported.

## Displays

The Toucan MIP uses SCK P1.13, MOSI P1.15, and active-high CS P0.03 at 1 MHz, with native 144×168 geometry and VCOM maintenance. Hardware references: [Toucan upgrade kit](https://shop.beekeeb.com/products/toucan-upgrade-kit) and [Beekeeb display overlay](https://github.com/beekeeb/zmk-keyboard-toucan/blob/main/boards/shields/nice_view_gem/nice_view_gem.overlay).

The shared [Prospector driver](../firmware/README.md) uses SPI2 at 4 MHz with the hardware-verified initialization sequence and active-high backlight. Display regression tests cover geometry, orientation, status rendering, and the rounded-corner title inset.
