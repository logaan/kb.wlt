# Milestone 4: Paintbrush OLED

This target adds the kit's 128×32 SSD1306 I²C OLED at address `0x3c`. SDA is nice!nano P0.17 (D2), SCL is P0.20 (D3), with the board's VCC and GND display pads. Display transfer/geometry tests pass on the host. The panel itself has not been tested.

- [ ] With power disconnected, confirm the OLED is fitted in the correct orientation and its pins match the board markings.
- [ ] Run `./milestones/04-paintbrush-display/build.sh` for Bluetooth with OLED. The Wavelet source is this folder's `braille.wlt`.
- [ ] Enter the bootloader and run `./milestones/04-paintbrush-display/flash.sh /Volumes/NICENANO`, using the actual mounted volume. For bootloader/handedness variants, use the corresponding suffixed UF2 as in milestone 2.
- [ ] Confirm the OLED displays a Bluetooth waiting/pairing state.
- [ ] Pair or reconnect as in milestone 3. Confirm the display changes to **kb.wlt BLE**.
- [ ] Hold a dot and confirm its number and braille-circle indicator appear on the display.
- [ ] Roll a multi-dot chord; confirm the held-key labels follow physical releases while the accumulated chord remains until the last release.
- [ ] Type `a`, uppercase `A`, Space, and Return. Confirm the HID status line changes along with the typed output.
- [ ] Repeat rapid chords while watching the OLED; confirm display refresh does not interfere with typing.
- [ ] Power off, remove the OLED if socketed, and power on again. Confirm Bluetooth typing still works; display errors must not stop the keyboard.
- [ ] Optionally run `./milestones/04-paintbrush-display/build-usb.sh`, then `./scripts/flash-uf2.sh dist/paintbrush-display.uf2 /Volumes/NICENANO`. Repeat the display and typing checks over USB, with **kb.wlt USB** shown when configured.

The current status renderer is native Rust; braille behavior remains Wavelet. This panel target does not drive nice!view, 128×64 OLEDs, or the Prospector's color LCD.
