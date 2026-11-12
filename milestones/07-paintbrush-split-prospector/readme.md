# Milestone 7: Two Paintbrush halves through Prospector

Both Paintbrushes send their top-row keys to a Prospector over Bluetooth. The dongle combines their snapshots, runs this folder's Wavelet braille component, displays status, and sends USB HID to the computer. Three firmware images cross-compile; radio behavior remains to be confirmed on hardware.

- [ ] Confirm the top-row layout in [milestone 6](../06-paintbrush-split/readme.md).
- [ ] Run `./milestones/07-paintbrush-split-prospector/build.sh`.
- [ ] Enter all three bootloaders. Run `./milestones/07-paintbrush-split-prospector/flash.sh /Volumes/LEFT_NICENANO /Volumes/RIGHT_NICENANO /Volumes/XIAO` using the real volume names in that order.
- [ ] Keep Prospector on USB to the computer. Supply USB or battery power to both Paintbrushes.
- [ ] Wait for both Bluetooth links; neither satellite is paired through the host's Bluetooth settings.
- [ ] Confirm **kb.wlt Split Receiver** appears as a USB keyboard and the LCD renders status.
- [ ] Type `a` on the left, then dot 4 alone on the right, and verify both boards affect the display/output.
- [ ] Chord dots 1+4 across the boards and confirm `c` appears only after both release.
- [ ] Test uppercase `A`, Space, Space+7 Return, and dot-7 Backspace.
- [ ] Repeat cross-board chords quickly with different release orders; verify no duplicates or stuck keys.
- [ ] Turn off either satellite during a chord. Reconnect and release all keys; confirm the interrupted chord is discarded and subsequent chords work.
- [ ] Power-cycle all three boards and verify both links recover.
- [ ] Change this folder's `braille.wlt`, rebuild and reflash only `prospector-pair`, and confirm the updated behavior.

Default bootloader addresses and mirrored-assembly options are the same as milestones 5 and 6. Both satellites intentionally have no USB HID interface. The dongle must run kb.wlt rather than stock ZMK firmware.

## QuickBrailleFox menu

Follow the [Prospector menu controls and hardware checks](../05-paintbrush-prospector/readme.md#quickbraillefox-menu) using the combined braille layout.

- [ ] Open **qq**, **qr**, **qc**, **qb**, and **qd** with the namespace chords split across the two halves. Confirm the editor receives no keystrokes.
- [ ] Capture a trigger that spans both halves and a second binding with more than one chord. Confirm the saved bindings type their expansions in both braille modes.
- [ ] Restart the links and confirm diagnostics shows both peers after reconnecting. Clear pairings, let both halves re-pair, and verify braille preferences persist.
- [ ] Disconnect either half while navigating or capturing a trigger. Release every key before typing again and confirm the interrupted chord produces no host report.
