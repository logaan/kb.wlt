# Paintbrush

Two Paintbrush Bluetooth satellites send key snapshots to a USB-connected Prospector receiver. This is the only supported Paintbrush configuration. Standalone USB/Bluetooth HID keyboards, single-keyboard receivers, and OLED variants have been removed.

Board-specific entry points and split application logic live in `firmware/`; shared Nordic radio, USB, storage, and LCD support live in [firmware/](../firmware/). The canonical Wavelet client is [braille.wlt](../clients/components/braille/braille.wlt). One Wavelet component handles braille mapping, menus, and display scenes on the receiver. The shared Rust host handles chord state, UEB Grade 1 buffering, bindings, and HID reports.

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

For subsequent Prospector updates, keep it connected by USB and open **Main Menu → Debug → UF2 Bootloader**. Select **Prospector** and choose **Enter UF2**. Double-tap reset to enter UF2 on the Paintbrush halves or directly on Prospector.

Keep Prospector on USB and power the satellites from USB or batteries. Both halves pair automatically with the receiver; do not pair them through the computer's Bluetooth settings. The host sees **kb.wlt Split Receiver**. A satellite's USB connection only supplies power and bootloader access.

Only the receiver executes the client. For a custom client, use `cargo xtask build prospector-pair --source path/to/braille.wlt`; reflash only the receiver. Regenerate checked-in components with `./scripts/build-components.sh` before committing changes to the canonical client.

## Receiver menus

Hold Space and chord **⠟** to open the Braille menu. Release the dots to choose another chord while keeping Space held.

In menus, Space+**⠁** goes to the previous item, Space+**⠈** goes to the next item, Space+**⠑** opens the selected item, and Space+**⠃** goes back. **⠓** opens Help. Menu chords stay on the receiver. Release all keys before resuming typing. Settings and bindings save asynchronously. Bluetooth reconnection and pairing reset are available in Configuration.

The Prospector uses the shared landscape ST7789 driver and LVGL renderer. Touch, trackpad input, and automatic brightness are unavailable.
