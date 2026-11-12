# Milestone 10: Toucan 2 with nice!view and Prospector

Both Toucan halves become Bluetooth matrix satellites. Prospector combines their keys, runs this folder's Wavelet component, displays status, and sends USB HID to the computer. The left Toucan's nice!view remains active and receives keyboard status from Prospector. Radio and display behavior still require hardware confirmation.

- [ ] Run `./milestones/10-toucan-prospector/build.sh`.
- [ ] Enter the left, right, and Prospector bootloaders. Run `./milestones/10-toucan-prospector/flash.sh /Volumes/LEFT_XIAO /Volumes/RIGHT_XIAO /Volumes/PROSPECTOR` with their actual volume paths, in that order.
- [ ] Keep Prospector connected to the computer by USB; power both keyboard halves from USB or batteries.
- [ ] Wait for both halves to connect automatically. Do not pair the satellites through the computer's Bluetooth settings.
- [ ] Confirm the computer sees **kb.wlt Toucan Receiver** as a USB keyboard.
- [ ] Confirm the Prospector LCD displays connection, held-key count, and HID status.
- [ ] Confirm the left Toucan's nice!view also displays the receiver's keyboard status, including keys pressed on the right half.
- [ ] Repeat every key and thumb check from [milestone 8](../08-toucan-bluetooth/readme.md).
- [ ] Hold Shift or the layer key on one half while using the other; confirm modifiers and layers span both halves.
- [ ] Confirm the active layer and last HID output update on both displays.
- [ ] Disconnect one half during a held key or modifier; verify the host releases it, then reconnect and verify fresh typing.
- [ ] Reset Prospector while keys are held. Release all keys after reconnecting and confirm there is no phantom output.
- [ ] Confirm the nice!view clears stale key, layer, and HID status after losing Prospector, then resumes live updates after reconnection.
- [ ] Power-cycle all three controllers and confirm their saved pairing relationships recover.
- [ ] Edit this folder's `keymap.wlt`, rebuild and reflash only `toucan-prospector`, and confirm the changed behavior without reflashing either satellite.

The keymap matches milestone 8. Both the Prospector LCD and the left keyboard's nice!view are enabled; the trackpad remains inactive. All three default images target XIAO stock S140 v7 at `0x27000`; use per-target bootloader variants if your boards differ.
