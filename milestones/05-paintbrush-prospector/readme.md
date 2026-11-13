# Milestone 5: Paintbrush through Prospector

This build sends raw Paintbrush key snapshots over an encrypted Bluetooth link to the Prospector XIAO. The dongle executes this folder's Wavelet braille component and sends USB HID reports to the computer. The Prospector's ST7789 LCD displays connection, chord, and last-report status. Both firmware variants cross-compile; the complete radio/display path needs hardware testing.

- [ ] Complete the USB Paintbrush checks in milestone 2 first.
- [ ] Run `./milestones/05-paintbrush-prospector/build.sh`.
- [ ] Put the Paintbrush and Prospector into their UF2 bootloaders, then run `./milestones/05-paintbrush-prospector/flash.sh /Volumes/NICENANO /Volumes/XIAO` with their actual volume paths, in that order.
- [ ] Confirm the Prospector uses its stock XIAO S140 v7 bootloader (`0x27000`) and the nice!nano uses S140 v6 (`0x26000`). For different bootloaders, build each target directly with `cargo xtask build TARGET --source milestones/05-paintbrush-prospector/braille.wlt --nosd` or `--s140v7`/`--s140v6`, then flash its suffixed image.
- [ ] Keep the Prospector connected to the computer by USB. Power the Paintbrush from USB or its battery.
- [ ] Wait for the boards to discover and pair automatically. Do not pair the Paintbrush satellite through the computer's Bluetooth settings.
- [ ] Confirm the computer sees **kb.wlt Split Receiver** as a USB keyboard.
- [ ] Confirm the LCD turns on and changes from waiting to connected.
- [ ] In a US English text editor, type `a`, `b`, `cde`, uppercase `A`, Space, Space+7 Return, and dot-7 Backspace using milestone 2's layout.
- [ ] Confirm the LCD updates with held keys, accumulated dots, and the latest HID output.
- [ ] Power-cycle the Paintbrush, release all its keys, and confirm it reconnects and types again.
- [ ] Disconnect or power off the Paintbrush midway through a chord. Reconnect and release everything; confirm the interrupted chord is not emitted.
- [ ] Edit this folder's `braille.wlt`, rebuild the Prospector target, reflash the dongle, and confirm the changed mapping without reflashing the satellite.

The peripheral image intentionally has no USB keyboard interface; its USB cable supplies power and allows bootloader access. The dongle uses kb.wlt's split protocol and does not connect to stock ZMK peripherals. Automatic LCD brightness, touch, and the optional light sensor are not implemented.

## One-shot hardware checks

Follow the [milestone 2 one-shot procedure](../02-paintbrush-usb/readme.md#one-shot-hardware-checks) through Prospector in Computer 8 and UEB Grade 1.

- [ ] Shift arms immediately.
- [ ] Control arms immediately.
- [ ] Alt arms immediately.
- [ ] Command arms immediately.
- [ ] Globe arms immediately on the USB Consumer Control interface.
- [ ] Control and Shift stack for the next key and then release.
- [ ] The armed modifier expires after one second without another key.
- [ ] Globe plus `a` produces both host events and releases both.

## QuickBrailleFox menu

Hold Space and chord **q** (dots 1+2+3+4+5), release the dots, and chord **q** again within two seconds. Keep Space held between chords or release and press it again. To open a screen directly, replace the second **q** with the screen's shortcut, still holding Space.

| Shortcut | Screen |
| --- | --- |
| qq | Main menu |
| qr (r = dots 1+2+3+5) | Braille reference |
| qc (c = dots 1+4) | Configuration |
| qb (b = dots 1+2) | Chords and contractions |
| qd (d = dots 1+4+5) | Diagnostics |

In menus, Space+1 and Space+4 move the selection, Space+3 and Space+6 move a page, Space+7 selects, and Space+12 goes back. Space+12 from the main menu returns to typing. All menu chords stay on the Prospector. Release every key after exiting or reconnecting before typing again.

### Reference

In Computer 8, the LCD shows the active Wavelet component's nonzero mappings, with Space labeled separately. Some eight-dot mappings require a physical dot-8 key that the default Paintbrush layout replaces with Space. The Prospector holds up to 24 taps from a custom component until the chord finishes. A component exceeding that limit raises a fault. In UEB Grade 1, Space+g opens the sign guide, including multi-cell and context-dependent signs, and Space+g again returns to the single-cell map. A single-chord override appears in the reference map. Find all saved triggers in **qb**.

| Reference control | Action |
| --- | --- |
| Space + dot 1 / dot 4 | Previous / next mapping |
| Space + dot 3 / dot 6 | Previous / next page |
| Space + dots 1+2+3 / 4+5+6 | First / last mapping |
| A plain chord | Jump to that mapping |
| Space + f (dots 1+2+4), then any chord | Look up that exact chord |
| Space + dots 1+2 | Exit reference |

### Configuration and bindings

Select the braille mode to switch between Computer 8 and UEB Grade 1. The Bluetooth actions restart the Prospector’s wireless keyboard links or clear saved pairings and pair again. Pairing reset preserves the Prospector’s braille mode and bindings. The USB host connection stays active while the keyboards reconnect.

The binding editor has 12 slots. Select a slot and a trigger length from one to four chords, then press and release each trigger chord. Type up to 24 characters using Computer 8. Dot 7 erases, Space+7 applies the edit, and Space+12 cancels. Choose **Edit expansion** to change the text while retaining the trigger, or **Delete this binding** to clear the slot. Space+q is reserved. Space+q followed by Space+q returns to the main menu during trigger capture.

The mode and bindings take effect immediately and save asynchronously. The configuration and diagnostics screens show an unsaved warning until storage succeeds. A complete binding that also starts a longer trigger waits up to 1.2 seconds for the remaining chords.

### Menu hardware checks

- [ ] Open each screen both with Space held and with full releases between chords. Navigate, enter text in the binding editor, cancel, and return to typing with no host output from the menus.
- [ ] Browse to the last Computer 8 mapping, inspect six-dot and eight-dot cells, and look up Space+1 with literal lookup. Change a mapping in this milestone's Wavelet source, rebuild the dongle, and confirm the reference changes.
- [ ] Switch to UEB Grade 1. Check a capital, a number, and a multi-cell punctuation sign against the on-device guide.
- [ ] Save a one-chord override and a multi-chord contraction. Confirm both expand in a US English text editor. Edit the expansion and delete one slot.
- [ ] Wait for the unsaved indicator to clear, then power-cycle the Prospector and confirm the mode and remaining binding persist.
- [ ] Restart the Bluetooth links, then clear saved pairings and confirm automatic re-pairing. Verify the braille mode and bindings persist.
- [ ] Exit a menu with Space held. Press another chord, then release everything. Confirm no Space or partial chord reaches the editor.
- [ ] Disconnect a satellite while a menu chord is held. Reconnect with keys held and confirm typing resumes after all keys release.
- [ ] With a test Wavelet component that traps, confirm diagnostics remains reachable and reports the fault.

Display hardware: [Beekeeb’s Prospector](https://shop.beekeeb.com/products/pre-soldered-prospector-zmk-dongle) includes the Waveshare 1.69-inch Touch LCD Module, 240×280 IPS. [Waveshare](https://www.waveshare.com/product/displays/lcd-oled/lcd-oled-3/1.69inch-touch-lcd-module.htm) identifies the display controller as ST7789V2 and the unused touch controller as CST816T. The firmware uses RGB565 over SPI; touch is inactive.

The [upstream XIAO overlay](https://github.com/carrefinho/prospector-zmk-module/blob/main/boards/shields/prospector_adapter/boards/seeeduino_xiao_ble.overlay) matches the firmware’s 240×280 window with a 20-row offset: SCK P1.13, MOSI P1.15, CS P1.14 (D9), DC P1.12 (D7), reset P0.29 (D3), and inverted backlight P1.11 (D6). Confirm orientation, colors, screen edges, and backlight on the physical unit; source verification is not hardware validation.
