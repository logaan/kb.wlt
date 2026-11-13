# Paintbrush

Paintbrush USB/Bluetooth braille keyboard applications and their single- or two-half Prospector receivers. Board-specific entry points, OLED integration, and split application logic live in `firmware/`; shared Nordic radio, USB, storage, and LCD support live in [firmware/](../firmware/).

The canonical Wavelet client is [braille.wlt](../clients/components/braille/braille.wlt). The Rust host supplies chord processing, Computer 8 and UEB Grade 1 typing, one-shot modifiers, and receiver menus. These configurations remain available for development; acceptance of the Toucan hardware does not imply Paintbrush hardware validation.

## Builds

Run from the repository root:

```sh
cargo xtask build paintbrush-usb
cargo xtask build paintbrush-ble
cargo xtask build paintbrush-satellite
cargo xtask build prospector-single
```

Choose the configuration you intend to flash:

| Configuration | Targets |
| --- | --- |
| Standalone USB or Bluetooth | `paintbrush-usb` or `paintbrush-ble` |
| Standalone with OLED | `paintbrush-display` or `paintbrush-ble-display` |
| One Paintbrush through USB Prospector | `paintbrush-satellite` and `prospector-single` |
| Two halves through USB Prospector | `paintbrush-left-satellite`, `paintbrush-right-satellite`, and `prospector-pair` |
| Split with the keyboard itself as USB receiver | `paintbrush-central` and `paintbrush-right-satellite` |

`paintbrush-usb-left` and `paintbrush-ble-left` select the left-hand layout. See `cargo xtask targets` for the complete catalog. For a custom client, add `--source path/to/braille.wlt` to the application target build. In receiver configurations only the receiver executes the client; satellites send key snapshots.

## Flash and connect

Enter each board's UF2 bootloader and flash one board at a time:

```sh
./scripts/flash-uf2.sh dist/paintbrush-satellite.uf2 /Volumes/NICENANO
./scripts/flash-uf2.sh dist/prospector-single.uf2 /Volumes/XIAO-SENSE
```

Use the actual mounted volume paths. Paintbrush defaults to nice!nano S140 v6 at `0x26000`; Prospector defaults to XIAO S140 v7 at `0x27000`. `--s140v6`, `--s140v7`, and `--nosd` select alternatives and suffixed outputs for compatible bootloaders.

Keep Prospector on USB and power the satellites from USB or batteries. Satellites automatically pair with the receiver; do not pair them through the host computer's Bluetooth settings. The host sees **kb.wlt Split Receiver**. A satellite's USB connection only supplies power and bootloader access.

## Receiver menus

Hold Space, chord **q** (dots 1+2+3+4+5), release the dots, then chord **q** again within two seconds. For direct access, replace the second chord with **r** (reference), **c** (configuration), **b** (bindings), or **d** (diagnostics).

In menus, Space+1/4 moves the selection, Space+3/6 changes pages, Space+7 selects, and Space+12 goes back. Menu chords stay on the receiver. Release all keys before resuming typing. Configuration and bindings save asynchronously; wait for the unsaved indicator to clear before powering off.

Prospector uses the shared ST7789 LCD driver. Brightness adjustment, touch, and light sensing are not implemented. Software SPI color diagnosis is available through `cargo xtask build prospector-display-test`; restore the appropriate receiver image after testing.
