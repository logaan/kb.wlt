# Milestone 2: Paintbrush USB braille

The nice!nano v2 firmware cross-compiles. Host tests cover all braille masks, chord rollover, debounce, and HID press/release reports. Complete these unchecked steps on real hardware.

The default build uses the standard right-hand Paintbrush wiring. Viewed as two rows from left to right:

| Row | First | Second | Third | Fourth |
| --- | --- | --- | --- | --- |
| Top | dot 7 | dot 3 | dot 2 | dot 1 |
| Bottom | dot 4 | dot 5 | dot 6 | braille Space |

The physical key in the dot-8 position is Space. Space + dot 7 is Return. Dot 7 alone is Backspace. Chords emit after every participating key has been released. Character mappings match the computer8 printable ASCII table in `zmk-braille-module`; the host keyboard layout should be US English.

- [ ] Confirm Rust's `thumbv7em-none-eabihf` target is installed and `wavelet` is on PATH (or set `WAVELET`).
- [ ] Run `./milestones/02-paintbrush-usb/build.sh`. The source is this folder's `braille.wlt`; the output is `dist/paintbrush-usb.uf2`.
- [ ] If the board is assembled for the left hand, build with `build.sh --left` and use `dist/paintbrush-usb-left.uf2`.
- [ ] Connect USB, double-press RESET, and identify the nice!nano UF2 volume using `INFO_UF2.TXT`.
- [ ] Confirm the bootloader uses the stock S140 v6 application address `0x26000`. For `nosd`, build with `--nosd`; for S140 v7, build with `--s140v7`. Use the correspondingly suffixed UF2 image.
- [ ] Run `./milestones/02-paintbrush-usb/flash.sh /Volumes/NICENANO` with the actual volume name. For another variant, use `./scripts/flash-uf2.sh dist/VARIANT.uf2 /Volumes/ACTUAL_VOLUME`.
- [ ] Confirm the host enumerates a USB keyboard named **Paintbrush Braille**.
- [ ] Open a text editor using the US English keyboard layout.
- [ ] Press and release dot 1: `a` appears once.
- [ ] Hold dots 1+2 together, release one while keeping the other down: nothing appears until the last release, then `b` appears once.
- [ ] Type dots 1+4, then dots 1+4+5, then dots 1+5: the result is `cde`.
- [ ] Press dot 7 with dot 1: `A` appears.
- [ ] Press Space alone: a space appears.
- [ ] Press Space + dot 7: a newline appears.
- [ ] Press dot 7 alone: the previous character is deleted.
- [ ] Repeat `a` quickly several times and confirm there are no stuck keys or duplicate characters.
- [ ] Hold a chord while unplugging USB; reconnect and release it. Confirm no phantom character appears, then confirm fresh chords work.
- [ ] Edit a mapping in this folder's `braille.wlt`, rebuild and flash, and confirm your edited output.

## One-shot hardware checks

Use a HID event monitor that can show keyboard modifiers and Consumer Control usage. Release all keys in each chord before entering the next chord.

- [ ] Chord Space+4+7 and verify that left Shift is pressed immediately.
- [ ] Chord Space+3+7 and verify that left Control is pressed immediately.
- [ ] Chord Space+2+7 and verify that left Alt is pressed immediately.
- [ ] Chord Space+1+7 and verify that left GUI (Command) is pressed immediately.
- [ ] Chord Space+5+7 and verify that Globe is pressed on the Consumer Control interface (usage `0x029D`).
- [ ] Arm Control, then Shift, then type `a` within one second of the first chord. Verify Control+Shift with `a`, then type another `a` and verify neither modifier remains.
- [ ] Arm Shift and wait a second without typing. Verify Shift releases, then type `a` and verify it is unmodified.
- [ ] Arm Globe and type `a` within one second. Verify the `a` key press while Globe is held and that both interfaces return to neutral.
