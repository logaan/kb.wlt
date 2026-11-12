# Hardware validation

These are implementation candidates verified with host tests and nRF52840 cross-builds. No real hardware behavior has been confirmed. Work through each folder's checklist and check the milestones here only after its hardware checks pass.

- [ ] [1. XIAO Sense Plus: Wavelet LED cycle and composed brightness middleware](01-xiao-led/readme.md)
- [ ] [2. One Paintbrush: USB braille, seven dots plus Space](02-paintbrush-usb/readme.md)
- [ ] [3. Paintbrush: Bluetooth HID](03-paintbrush-bluetooth/readme.md)
- [ ] [4. Paintbrush: OLED status display](04-paintbrush-display/readme.md)
- [ ] [5. Paintbrush through Prospector, including braille reference](05-paintbrush-prospector/readme.md)
- [ ] [6. Two Paintbrushes: split braille over USB](06-paintbrush-split/readme.md)
- [ ] [7. Two Paintbrushes through Prospector, including braille reference](07-paintbrush-split-prospector/readme.md)
- [ ] [8. Toucan 2: Bluetooth keyboard](08-toucan-bluetooth/readme.md)
- [ ] [9. Toucan 2: nice!view display](09-toucan-display/readme.md)
- [ ] [10. Toucan 2 through Prospector](10-toucan-prospector/readme.md)
- [ ] [Third XIAO reset controller: wiring and independent bootloader entry](../resetter/README.md)

Build commands compile the folder's Wavelet example and embed it into the firmware. Release bundles also contain ready-made UF2 files in `dist/`. Identify each board's bootloader volume before copying an image, especially when several controllers are connected at once.
