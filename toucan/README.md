# Toucan

Firmware for the Toucan 1 with two XIAO nRF52840 controllers and the 144×168 MIP display. The left half runs the Wavelet keymap, braille, and display interfaces in one instance. The right half sends matrix snapshots to the left. The Prospector relays HID reports to USB and draws the forwarded display model with its own instance.

## Build and flash

Run from the repository root:

```sh
cargo xtask build toucan-left-satellite-display
cargo xtask build toucan-right-satellite
cargo xtask build toucan-prospector
```

Flash all three matching images for the initial upgrade. Enter each board's bootloader and substitute its mounted volume.

```sh
./scripts/flash-uf2.sh dist/toucan-left-satellite-display.uf2 /Volumes/XIAO-BOOT
./scripts/flash-uf2.sh dist/toucan-right-satellite.uf2 /Volumes/XIAO-BOOT
./scripts/flash-uf2.sh dist/toucan-prospector.uf2 /Volumes/XIAO-SENSE
```

The stock XIAO S140 v7 bootloader loads at `0x27000`. The `--s140v6` and `--nosd` build options select alternate origins for matching bootloaders and add suffixes to the output files.

A change confined to the conventional keymap runs on the left half. For a one-off build, run `cargo xtask build toucan-left-satellite-display --source path/to/keymap.wlt`. Build the left half and Prospector together after changing display behavior or shared contracts. Regenerate checked-in artifacts after editing Wavelet code. [Canonical Wavelet source](../clients/components/toucan-keymap/toucan-keymap.wlt)

## Connect and type

Fresh installs use Prospector USB. Connect the Prospector to the computer with a data cable, then power both halves. The right half pairs with the left and the left pairs with Prospector automatically. The computer sees **kb.wlt Toucan Receiver** over USB.

To type directly over Bluetooth, hold all three left thumb keys for two seconds. Pair **kb.wlt Toucan** with the computer using its Bluetooth settings. The halves work together while the Prospector is off. Hold the three left thumbs again to return to Prospector USB. The left half saves the selected route with its settings.

Tap the outer left thumb to switch between QWERTY and Braille. Hold it to use Braille momentarily. The cells below correspond to the named keys. Use the normal Space thumb for Braille Space.

```text
A  S  D  F      J  K  L  ;
⡀  ⠄  ⠂  ⠁      ⠈  ⠐  ⠠  ⢀
```

Chord keys together and release to type. Hold **F** to preview **a** before releasing it. The display previews the active Computer 8 or UEB Grade 1 result, including saved bindings. Buffered cells appear oldest first.

Hold Space and chord **⠟** twice to open the main menu. Release the other keys between cells. Space+**⠁** goes to the previous item, Space+**⠈** goes to the next item, Space+**⡀** opens the selected item, and Space+**⠃** goes back. **⠓** opens Help. Reference, Configuration, and Bindings are available on the left MIP when using either transport.

## Displays and hardware status

The MIP uses SCK P1.13, MOSI P1.15, and active-high CS P0.03 at 1 MHz with VCOM maintenance. The Prospector uses SPI2 at 4 MHz and renders 280×240 landscape. Both use Wavelet scenes rendered through LVGL. Trackpad input is unavailable. [Toucan upgrade kit](https://shop.beekeeb.com/products/toucan-upgrade-kit) · [Beekeeb display overlay](https://github.com/beekeeb/zmk-keyboard-toucan/blob/main/boards/shields/nice_view_gem/nice_view_gem.overlay) · [Prospector hardware](../firmware/README.md)

The previous USB-receiver firmware passed typing, display, cross-half modifier, reconnection, and power-cycle tests. The new pairing topology, direct Bluetooth, battery readings, and LVGL rendering still need hardware validation. The [browser emulator](../emulator/) exercises both typing routes and displays.
