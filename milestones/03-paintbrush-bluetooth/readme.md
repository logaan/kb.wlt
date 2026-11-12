# Milestone 3: Paintbrush Bluetooth

The firmware cross-compiles with encrypted Bluetooth HID and persistent pairing records. Radio operation, host interoperability, and power behavior need real hardware confirmation. Use the same key layout and computer8 behavior as [milestone 2](../02-paintbrush-usb/readme.md).

- [ ] Build with `./milestones/03-paintbrush-bluetooth/build.sh`. Add `--left`, `--nosd`, or `--s140v7` only if required by your assembly or bootloader; see milestone 2.
- [ ] Enter the nice!nano UF2 bootloader and run `./milestones/03-paintbrush-bluetooth/flash.sh /Volumes/NICENANO`, substituting the real volume. For a variant, flash its suffixed UF2 using `scripts/flash-uf2.sh`.
- [ ] Open the computer's Bluetooth settings and find **kb.wlt Paintbrush**.
- [ ] Pair it, then wait for the computer to report it connected.
- [ ] With a text editor focused and US English layout selected, repeat milestone 2's `a`, `b`, `cde`, uppercase `A`, Space, Return, and Backspace tests.
- [ ] Confirm each chord emits once, only on the last release, with no stuck modifiers.
- [ ] Disconnect and reconnect board power. Confirm it reconnects to the paired host without pairing again.
- [ ] Disable and re-enable the computer's Bluetooth. Confirm fresh chords work after reconnection.
- [ ] Hold a chord while disconnecting the host, release after reconnecting, and confirm no phantom character is sent.
- [ ] If a battery is installed, unplug USB and confirm typing still works over Bluetooth. Otherwise retain USB for power; this image sends keys over Bluetooth only.
- [ ] To clear stored pairing, hold all eight keys while resetting/powering on, continue holding for two seconds, then release. Forget the old device on the host and pair again.
- [ ] Edit this folder's `braille.wlt`, rebuild and flash, and confirm the custom mapping works over Bluetooth.

Bond data lives in two reserved flash pages and is retained by ordinary application flashing. Pairing uses Just Works; the board has no passcode display or confirmation button. Battery-life tuning and low-power sleep are not yet hardware-validated.

## One-shot hardware checks

When upgrading a previously paired device, clear the Paintbrush's saved pairing using the reset chord above, forget the device on the host, and pair again so the host reads the updated Bluetooth HID report map.

Follow the [milestone 2 one-shot procedure](../02-paintbrush-usb/readme.md#one-shot-hardware-checks) over Bluetooth.

- [ ] Shift arms immediately.
- [ ] Control arms immediately.
- [ ] Alt arms immediately.
- [ ] Command arms immediately.
- [ ] Globe arms immediately on the Consumer Control report.
- [ ] Control and Shift stack for the next key and then release.
- [ ] The armed modifier expires after one second without another key.
- [ ] Globe plus `a` produces both host events and releases both.
