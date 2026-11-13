# Milestone 10: Toucan (upgraded Toucan 1) with MIP display and Prospector

Both Toucan halves become Bluetooth matrix satellites. Prospector combines their
keys, runs this folder's Wavelet component, displays status, and sends USB HID
to the computer. The left Toucan's MIP display remains active and receives
keyboard status from Prospector. Radio and display behavior still require
hardware confirmation.

- [x] Run `./milestones/10-toucan-prospector/build.sh`. Flash one board at a
  time: each XIAO bootloader mounts at `/Volumes/XIAO-SENSE`. Disconnect the
  flashed board before connecting the next. Run these commands from the
  repository root.

- [x] Connect the left Toucan and enter its bootloader. Run
  `./scripts/flash-uf2.sh dist/toucan-left-satellite-display.uf2 /Volumes/XIAO-BOOT`.

- [x] Connect the right Toucan and enter its bootloader. Run
  `./scripts/flash-uf2.sh dist/toucan-right-satellite.uf2 /Volumes/XIAO-BOOT`.

- [x] Connect Prospector and enter its bootloader. Run
  `./scripts/flash-uf2.sh dist/toucan-prospector.uf2 /Volumes/XIAO-SENSE`.

- [x] Keep Prospector connected to the computer by USB; power both keyboard
  halves from USB or batteries.

- [x] Wait for both halves to connect automatically. Do not pair the satellites
  through the computer's Bluetooth settings.

- [x] Confirm the computer sees **kb.wlt Toucan Receiver** as a USB keyboard.

- [ ] Confirm the Prospector LCD displays connection, held-key count, and HID
  status.

- [ ] Confirm the left Toucan's MIP display also shows the receiver's keyboard
  status, including keys pressed on the right half.

- [ ] Check every ordinary key and thumb against the two-layer layout below.
  Hold the left outer thumb and check the layer's navigation keys. Braille
  bindings are intentionally inactive in this bindings-only version.

- [ ] Hold Shift or the layer key on one half while using the other; confirm
  modifiers and layers span both halves.

- [ ] Confirm the active layer and last HID output update on both displays.

- [ ] Disconnect one half during a held key or modifier; verify the host
  releases it, then reconnect and verify fresh typing.

- [ ] Reset Prospector while keys are held. Release all keys after reconnecting
  and confirm there is no phantom output.

- [ ] Confirm the MIP display clears stale key, layer, and HID status after
  losing Prospector, then resumes live updates after reconnection.

- [ ] Power-cycle all three controllers and confirm their saved pairing
  relationships recover.

- [ ] Edit this folder's `keymap.wlt`, rebuild and reflash only
  `toucan-prospector`, and confirm the changed behavior without reflashing
  either satellite.

The keymap imports the two-layer bindings from
`~/Code/logaan/all-my-keyboards/toucan/config/toucan.keymap`.
Each row is read left to right:

| Row | Left half | Right half |
| --- | --- | --- |
| Top | Tab Q W E R T | Y U I O P Esc |
| Home | Left Control A S D F G | H J K L Semicolon Apostrophe |
| Bottom | Left Shift Z X C V B | N M Comma Period Slash Minus |
| Thumbs | Braille layer, Backspace, Left GUI | Space, Return, Left Shift |

Hold the left outer thumb to access the second layer:

| Row | Left half | Right half |
| --- | --- | --- |
| Top | Computer 8, Tab, Left, Esc, Up, UEB1 | UEB2, Tab, Down, Right, Base, Inactive |
| Home | Inactive, Dot 7, Dot 3, Dot 2, Dot 1, Settings | Shorthand, Dot 4, Dot 5, Dot 6, Dot 8, Inactive |
| Bottom | Inactive, Left Shift, Left Control, Left Alt, Backspace, Inactive | Inactive, Left GUI, Return, Space, Inactive, Inactive |
| Thumbs | Base, Backspace, Left GUI | Braille Space, Return, Next mode |

This is a bindings-only port: dot keys, Braille Space, mode selectors,
Settings, and Base are named in `keymap.wlt` but emit no HID output. Release
the layer thumb to return to the ordinary layout. No braille engine integration
or ZMK tap-to-latch behavior is included. GUI means Command on macOS or
Windows/Super on other hosts. Layer indicators still use
the firmware's original rightmost-thumb detection, not this layer binding.

Both the Prospector LCD and the left keyboard's MIP display are enabled;
the trackpad remains inactive. All three default images
target XIAO stock S140 v7 at `0x27000`; use per-target bootloader variants if
your boards differ.

Display specifications and source checks are recorded in
[milestone 9](../09-toucan-display/readme.md) for the retained 144×168 Toucan
MIP panel and [milestone 5](../05-paintbrush-prospector/readme.md) for the
240×280 ST7789V2 Prospector LCD.
