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

## Braille reference

Hold Space and chord **q** (dots 1+2+3+4+5), release the dots, then chord **r** (dots 1+2+3+5) within two seconds. Keep Space held throughout, or release and press Space for each chord. The namespace is mnemonic: **QuickBrailleFox → reference**. Space+q then Space+c is reserved for configuration; unknown namespace commands are swallowed.

The LCD lists every nonzero mapping returned by the active Wavelet component, including custom mappings after a rebuild. It shows six-dot cells when possible, eight-dot cells when dot 7 or 8 is used, and labels Space separately. The stock component has 111 mappings. Some eight-dot mappings require a physical dot-8 key that the default Paintbrush layout replaces with Space.

| Reference control | Action |
| --- | --- |
| Space + dot 1 / dot 4 | Previous / next mapping |
| Space + dot 3 / dot 6 | Previous / next page |
| Space + dots 1+2+3 / 4+5+6 | First / last mapping |
| A plain chord | Jump to that mapping, or show unmapped |
| Space + f (dots 1+2+4), then any chord | Look up that exact chord, including navigation controls |
| Space + dots 1+2 | Exit reference |

All reference input is captured. After exiting, timing out in the namespace, or losing a connection, release every key before typing again. Space+q is reserved by the Prospector interface even if a custom component maps it. Hardware checks:

- [ ] Enter reference both with Space held continuously and with full releases between chords; confirm neither entry sequence types anything.
- [ ] Browse through the last mapping, inspect dot-7/dot-8 cells and Space mappings, and use literal lookup for Space+1.
- [ ] Exit while keeping Space held, press another chord, then release everything; confirm no Space or partial chord leaks into the editor.
- [ ] Disconnect a satellite during reference and reconnect with keys held; confirm typing resumes only after all keys release.
- [ ] Change a mapping in this milestone's `braille.wlt`, rebuild the dongle, and confirm its reference entry changes.
