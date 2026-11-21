# Toucan

Firmware for the Toucan 1 with two XIAO nRF52840 controllers and the 144×168 MIP display. The left half runs the Wavelet keymap, braille, and display interfaces in one instance. The right half sends matrix snapshots to the left. The Prospector relays HID reports to USB when selected and draws the forwarded display model with its own instance.

## Build and flash

Run from the repository root:

```sh
cargo xtask build toucan-left-satellite-display
cargo xtask build toucan-right-satellite
cargo xtask build toucan-prospector
```

Flash all three matching images for the initial upgrade. Enter each board's bootloader and substitute its mounted volume.

```sh
./scripts/flash dist/toucan-left-satellite-display.uf2 /Volumes/XIAO-BOOT
./scripts/flash dist/toucan-right-satellite.uf2 /Volumes/XIAO-BOOT
./scripts/flash dist/toucan-prospector.uf2 /Volumes/XIAO-SENSE
```

The stock XIAO S140 v7 bootloader loads at `0x27000`. The `--s140v6` and `--nosd` build options select alternate origins for matching bootloaders and add suffixes to the output files.

For subsequent updates, connect the target boards to USB with data cables. In Braille mode open **Main Menu → Debug → UF2 Bootloader**, select the boards, and choose **Enter UF2**. The right half and Prospector are available while linked to the left. A [touch-wired Prospector](../firmware/README.md#prospector-display) can use its own menu to enter UF2 while disconnected from the keyboard. If some remote requests fail, boards that accepted the request enter UF2 and the left half stays running. Double-tap a board’s reset button to enter UF2 directly.

A change confined to the conventional keymap runs on the left half. For a one-off build, run `cargo xtask build toucan-left-satellite-display --source path/to/keymap.wlt`. Build the left half and Prospector together after changing display behavior or shared contracts. Regenerate checked-in artifacts after editing Wavelet code. [Canonical Wavelet source](../clients/components/toucan-keymap/toucan-keymap.wlt)

## Connect and type

Fresh installs use Prospector USB. Connect the Prospector to the computer with a data cable, then power both halves. The right half pairs with the left and the left pairs with Prospector automatically. The computer sees **kb.wlt Toucan Receiver** over USB.

To change hosts, open the Braille menu and chord **t** for **Target**. Move to **Prospector**, **BT 1–5**, or **USB**, then open the selected item. The left half saves the choice. Existing direct Bluetooth pairings carry over to BT 1. Both displays show the selected host and its connection state while Prospector is linked.

- **Prospector** sends keystrokes to **kb.wlt Toucan Receiver** over the Prospector USB cable.
- **BT 1–5** each remember one computer. Select an empty slot and pair **kb.wlt Toucan** in the computer’s Bluetooth settings. The two halves can work with Prospector powered off. If another paired computer keeps trying to reconnect while you pair a new slot, temporarily turn off Bluetooth on that computer.
- **USB** sends keystrokes over the left half’s USB data cable to **kb.wlt Toucan USB**. This USB device can appear whenever the left half is cabled. Typing goes to it when **USB** is selected.

Tap either outer thumb to switch between QWERTY and Braille. Hold either outer thumb for momentary Braille. Each half has all eight dots, and either inner thumb sends Space in Braille. The left inner thumb remains the GUI key in QWERTY.

| Physical keys | Braille cells, left to right |
| --- | --- |
| A S D F | ⡀ ⠄ ⠂ ⠁ |
| Z X C V | ⢀ ⠠ ⠐ ⠈ |
| J K L ; | ⠈ ⠐ ⠠ ⢀ |
| M , . / | ⠁ ⠂ ⠄ ⡀ |

Pressing the same dot on both halves counts as one dot, held until both keys are released.

Chord keys together and release to type. Keys released less than 50 ms before the final release still count toward the chord. To correct an extra key, release it while holding the intended chord for at least 50 ms. Hold **F** to preview **a** before releasing it. The display previews the active Computer 8 or UEB Grade 1 result, including saved bindings. Buffered cells appear oldest first.

Chord **⠟̲** to open the Braille menu. An underlined cell includes Space. Keep Space held and release the other keys between menu chords. **⠁̲** goes to the previous item, **⠈̲** goes to the next item, **⠑̲** opens the selected item, and **⠃̲** goes back. **(H)elp (⠓)** opens Help. Reference, Configuration, Bindings, and Target are available with any selected host.

## Displays and hardware status

The MIP uses SCK P1.13, MOSI P1.15, and active-high CS P0.03 at 1 MHz with VCOM maintenance. The Prospector uses SPI2 at 4 MHz and renders 280×240 landscape. Both use Wavelet scenes rendered through LVGL. On a touch-wired Prospector, tap to open the menu or selected item, swipe to navigate, and hold to go back. [Toucan upgrade kit](https://shop.beekeeb.com/products/toucan-upgrade-kit) · [Prospector hardware](../firmware/README.md)

The previous USB-receiver firmware passed typing, display, cross-half modifier, reconnection, and power-cycle tests. After flashing the current stock image, the left half enumerated as **kb.wlt Toucan USB**. ARM emulation and the browser verified the left-only Reference menu and UF2 request. Bluetooth, touch, and left-only menu operation still need physical testing.

Both displays use battery estimates derived from each half’s measured voltage. A reading can be unavailable before the first sample, when its half has no battery, or when a split link drops. The browser emulator exercises the host picker, menus, and both displays. [Emulator setup](../emulator/)
