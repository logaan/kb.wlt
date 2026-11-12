# Milestone 6: Two Paintbrush halves

The left board receives the right board over Bluetooth, runs the Wavelet braille component, and connects to the computer by USB. Both images cross-compile. Physical split behavior is unverified.

| Top row, viewed left to right | First | Second | Third | Fourth |
| --- | --- | --- | --- | --- |
| Left Paintbrush | dot 7 | dot 3 | dot 2 | dot 1 |
| Right Paintbrush | dot 4 | dot 5 | dot 6 | braille Space |

Bottom-row keys are unassigned in this initial split layout. The eighth logical key is Space, as in the single-board layout; Space+7 produces Return.

- [ ] Run `./milestones/06-paintbrush-split/build.sh`.
- [ ] Enter both bootloaders, then run `./milestones/06-paintbrush-split/flash.sh /Volumes/LEFT_NICENANO /Volumes/RIGHT_NICENANO` using the actual volumes. First is the USB central; second is the right satellite.
- [ ] Connect the left board to the host by USB, and supply power to the right board. Wait for their Bluetooth link.
- [ ] Confirm **kb.wlt Split Receiver** enumerates as a USB keyboard.
- [ ] Check each top-row physical position against the table. The default pin order assumes the standard right-hand Paintbrush wiring on each board. If a board is assembled with mirrored wiring, rebuild that role with `--left`; for example `cargo xtask build paintbrush-central --left --source milestones/06-paintbrush-split/braille.wlt`.
- [ ] Type dot 1 on the left: `a` appears once on release.
- [ ] Chord dot 1 on the left with dot 4 on the right: `c` appears once after both release.
- [ ] Repeat the cross-board chord with each possible release order; verify no partial letters.
- [ ] Chord dot 7 on the left with dot 1: uppercase `A` appears.
- [ ] Press Space on the right, then Space on the right with dot 7 on the left; verify Space and Return.
- [ ] Confirm bottom-row keys produce no characters.
- [ ] Disconnect the right board during a chord; reconnect it and release all keys. Confirm no phantom output and that fresh cross-board chords work.
- [ ] Reset both boards and confirm they reconnect without manual pairing.

Use the bootloader variants described in milestone 2 when required. This milestone's host connection is USB; Bluetooth carries the link between halves.
