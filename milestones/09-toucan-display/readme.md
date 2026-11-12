# Milestone 9: Toucan 2 with nice!view

This adds the left half's native 160×68 nice!view panel to milestone 8. The renderer uses portrait orientation and maintains the display's VCOM signal while idle. Host tests cover the pixel packing and panel protocol; physical rendering is unverified. The trackpad remains inactive.

- [ ] Complete the Bluetooth typing checks in [milestone 8](../08-toucan-bluetooth/readme.md).
- [ ] Run `./milestones/09-toucan-display/build.sh`.
- [ ] Enter both bootloaders and run `./milestones/09-toucan-display/flash.sh /Volumes/LEFT_XIAO /Volumes/RIGHT_XIAO` with the actual volume paths. The right image is the same satellite role as milestone 8.
- [ ] Confirm the left nice!view shows **kb.wlt**, the Bluetooth state, and the last HID report.
- [ ] Pair or reconnect **kb.wlt Toucan** and verify the display changes to connected.
- [ ] Press keys on either half and confirm the displayed active-key count changes.
- [ ] Hold the rightmost thumb layer key and confirm the layer indication changes for the stock keymap.
- [ ] Type modified characters and confirm the HID status updates without interrupting typing.
- [ ] Leave the board idle for several minutes; confirm the display remains stable.
- [ ] Repeat the power-cycle and interrupted-key reconnection checks from milestone 8.
- [ ] Edit this folder's `keymap.wlt`, rebuild and reflash the left half, and verify the changed keys.

The panel uses SPI clock P1.13, MOSI P1.15, and active-high CS P0.03, matching the Toucan 2 left shield. The layer indication describes the supplied keymap's thumb-layer convention; it is not introspection into arbitrary custom components. Use milestone 8's bootloader variants if needed.
