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
