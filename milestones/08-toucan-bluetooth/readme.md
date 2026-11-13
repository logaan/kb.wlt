# Milestone 8: Toucan (upgraded Toucan 1) Bluetooth keyboard

The left XIAO runs the Wavelet keymap, connects to the right half over
Bluetooth, and presents Bluetooth HID to the computer. Cross-builds,
matrix/packet tests, and keymap tests pass. Hardware operation is unverified.
This target uses the Toucan 2 42-key wiring; its original MIP display and
upgraded Azoteq trackpad are inactive. The hardware under test is a Toucan 1
with the Toucan 2 upgrade kit.

Default US English keymap, each row read left to right:

| Row    | Left half                       | Right half                         |
| ------ | ------------------------------- | ---------------------------------- |
| Top    | Tab Q W E R T                   | Y U I O P Backslash                |
| Home   | Esc A S D F G                   | H J K L Semicolon Apostrophe       |
| Bottom | Left GUI Z X C V B              | N M Comma Period Slash Right Shift |
| Thumbs | Left Shift, Left Control, Space | Return, Backspace, Layer 1         |

Layer 1 is held by the rightmost right thumb. It provides digits on the top row,
F1–F11 on the home row, and arrows on the right bottom row. The actual mappings
are in this folder's `keymap.wlt`. GUI means Command on macOS or Windows/Super
on other hosts.

- [ ] Run `./milestones/08-toucan-bluetooth/build.sh` with Wavelet 0.12.1 or
      newer on PATH.
- [ ] Enter both XIAO UF2 bootloaders, identify which volume belongs to each
      half, and run
      `./milestones/08-toucan-bluetooth/flash.sh /Volumes/LEFT_XIAO /Volumes/RIGHT_XIAO`
      in that order.
- [ ] Confirm both use their stock S140 v7 application address `0x27000`. For a
      different bootloader, build each role directly with
      `cargo xtask build TARGET --source milestones/08-toucan-bluetooth/keymap.wlt --nosd`
      or `--s140v6`, and flash that suffixed image.
- [ ] Power both halves. Pair **kb.wlt Toucan** through the computer's Bluetooth
      settings; the right half connects to the left automatically.
- [ ] Open a text editor with US English keyboard layout and type every letter,
      number-layer digit, and punctuation key.
- [ ] Test all six thumb keys, including Space, Return, Backspace, Shift,
      Control, and the held layer.
- [ ] Hold left Shift and type on the right half; verify uppercase output and
      clean modifier release.
- [ ] Hold a letter, enter or leave the layer, then release the letter; verify
      no stuck key.
- [ ] Hold the layer while pressing a right-hand arrow; confirm navigation
      works.
- [ ] Press more than six nonmodifier keys, then release them; confirm rollover
      recovers and ordinary typing resumes.
- [ ] Power off one half while a key is held. Reconnect it and release all keys;
      confirm there is no phantom key and fresh typing works.
- [ ] Power-cycle both halves and confirm both the split link and host pairing
      recover.
- [ ] Edit this folder's `keymap.wlt`, rebuild and reflash the left half, and
      confirm the change. The right satellite only scans keys.
- [ ] Confirm the trackpad sends no mouse events in this milestone.

USB supplies power and provides bootloader access for these images; the host
keyboard connection is Bluetooth. Pairing records are stored in reserved flash.
To recover stale pairings, use the `settings-reset-xiao` image on affected
boards, wait for its LED indication, then restore normal firmware and
forget/re-pair the host entry.

## Two-component example

The optional `toucan-ble-composed` target runs `shift.wlt` and `keymap.wlt` as
separate components with isolated memories and explicit state. Input travels
forward through the pipeline; output travels back through the middleware. This
example adds Shift to outgoing keys. It requires `wasm-opt` from Binaryen and
`qemu-system-arm` on PATH, in addition to Wavelet. `WASM_OPT` and
`QEMU_SYSTEM_ARM` can specify absolute executable paths.

- [ ] Run `./milestones/08-toucan-bluetooth/build-composed.sh`. Confirm the ARM
      heap probe passes before firmware is produced.
- [ ] Flash both halves with
      `./milestones/08-toucan-bluetooth/flash-composed.sh /Volumes/LEFT_XIAO /Volumes/RIGHT_XIAO`.
- [ ] Type letter keys with Caps Lock off and confirm uppercase output without
      holding Shift.
- [ ] Release every key, then press a number-layer key and confirm its shifted
      symbol. Verify no modifier remains stuck after release or reconnect.
- [ ] Change the middleware, rebuild, and verify its effect independently of the
      keymap.
- [ ] Restore the ordinary milestone image after testing the example if desired.

The two-component target uses a 196 KiB heap and leaves at least 16 KiB for the
native stack in the linked image. Display features are rejected for this target.
Actual stack high-water use and hardware behavior remain unmeasured. The
separate `toucan-dongle-composed` target offers the same pipeline through USB
with two satellites and no LCD.
