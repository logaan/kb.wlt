# Paintbrush

Two Paintbrush Bluetooth satellites send key snapshots to a USB-connected Prospector receiver. This is the only supported Paintbrush configuration. Standalone USB/Bluetooth HID keyboards, single-keyboard receivers, and OLED variants have been removed.

Board-specific entry points and split application logic live in `firmware/`; shared Nordic radio, USB, storage, and LCD support live in [firmware/](../firmware/). The canonical Wavelet client is [braille.wlt](../clients/components/braille/braille.wlt). The Rust host supplies chord processing, Computer 8 and UEB Grade 1 typing, one-shot modifiers, and receiver menus.

This setup remains available for development and future hardware verification; acceptance of the Toucan hardware does not imply Paintbrush hardware validation.

## Build and flash

Run from the repository root:

```sh
cargo xtask build paintbrush-left-satellite
cargo xtask build paintbrush-right-satellite
cargo xtask build prospector-pair
```

Enter each board's UF2 bootloader and flash one board at a time, substituting the actual mounted volume:

```sh
./scripts/flash-uf2.sh dist/paintbrush-left-satellite.uf2 /Volumes/NICENANO
./scripts/flash-uf2.sh dist/paintbrush-right-satellite.uf2 /Volumes/NICENANO
./scripts/flash-uf2.sh dist/prospector-pair.uf2 /Volumes/XIAO-SENSE
```

Paintbrush defaults to nice!nano S140 v6 at `0x26000`; Prospector defaults to XIAO S140 v7 at `0x27000`. `--s140v6`, `--s140v7`, and `--nosd` select alternative origins and suffixed outputs for compatible bootloaders.

Keep Prospector on USB and power the satellites from USB or batteries. Both halves pair automatically with the receiver; do not pair them through the computer's Bluetooth settings. The host sees **kb.wlt Split Receiver**. A satellite's USB connection only supplies power and bootloader access.

Only the receiver executes the client. For a custom client, use `cargo xtask build prospector-pair --source path/to/braille.wlt`; reflash only the receiver. Regenerate checked-in components with `./scripts/build-components.sh` before committing changes to the canonical client.

## Receiver menus

Hold Space, chord **q** (dots 1+2+3+4+5), release the dots, then chord **q** again within two seconds. For direct access, replace the second chord with **r** (reference), **c** (configuration), **b** (bindings), or **d** (diagnostics).

In menus, Space+1/4 moves the selection, Space+3/6 changes pages, Space+7 selects, and Space+12 goes back. Menu chords stay on the receiver. Release all keys before resuming typing. Configuration and bindings save asynchronously; wait for the unsaved indicator to clear before powering off. Bluetooth reconnection and pairing reset remain available in the receiver's configuration menu; separate reset firmware is not required.

Prospector uses the shared ST7789 LCD driver. Brightness adjustment, touch, and light sensing are not implemented.
