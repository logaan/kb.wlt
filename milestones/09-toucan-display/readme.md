# Milestone 9: Toucan (upgraded Toucan 1) with MIP display

This adds the left half's native 144×168 monochrome MIP panel to milestone 8. The renderer uses portrait orientation and maintains the display's VCOM signal while idle. Host tests cover the pixel packing and panel protocol; physical rendering is unverified. The trackpad remains inactive.

- [ ] Complete the Bluetooth typing checks in [milestone 8](../08-toucan-bluetooth/readme.md).
- [ ] Run `./milestones/09-toucan-display/build.sh`.
- [ ] Enter both bootloaders and run `./milestones/09-toucan-display/flash.sh /Volumes/LEFT_XIAO /Volumes/RIGHT_XIAO` with the actual volume paths. The right image is the same satellite role as milestone 8.
- [ ] Confirm the full-width **kb.wlt / TOUCAN** header, transport and link indicator, large **KEYS HELD** and **LAYER** values, and separate **LAST HID / HEX** section fit the display with upright text.
- [ ] Pair or reconnect **kb.wlt Toucan** and verify the display changes to connected.
- [ ] Press keys on either half and confirm the large active-key count changes; check two-digit counts as well as zero.
- [ ] Hold the rightmost thumb layer key and confirm the layer indication changes for the stock keymap.
- [ ] Type modified characters and confirm the HID status updates without interrupting typing.
- [ ] Leave the board idle for several minutes; confirm the display remains stable.
- [ ] Repeat the power-cycle and interrupted-key reconnection checks from milestone 8.
- [ ] Edit this folder's `keymap.wlt`, rebuild and reflash the left half, and verify the changed keys.

The panel uses SPI clock P1.13, MOSI P1.15, and active-high CS P0.03, matching the Toucan (upgraded Toucan 1) left shield. The layer indication describes the supplied keymap's thumb-layer convention; it is not introspection into arbitrary custom components. Use milestone 8's bootloader variants if needed.

Hardware reference: the [original Toucan product](https://shop.beekeeb.com/products/toucan-wireless-piantor-wireless-split-keyboard-with-touchpad) includes a memory-in-pixel display. The [upgrade kit](https://shop.beekeeb.com/products/toucan-upgrade-kit) changes the trackpad and right case, retaining the original display. Beekeeb’s [display overlay](https://github.com/beekeeb/zmk-keyboard-toucan/blob/main/boards/shields/nice_view_gem/nice_view_gem.overlay) specifies `sharp,ls0xx`, 144×168, and 1 MHz SPI; its [left shield](https://github.com/beekeeb/zmk-keyboard-toucan/blob/main/boards/shields/toucan/toucan_left.overlay) supplies the pins above. The exact panel part number is unconfirmed. The separately sold [MIP module](https://shop.beekeeb.com/products/memory-in-pixel-lcd-display-module) is explicitly not a Toucan replacement.

The internal `nice_view` module and `nice-view` feature retain their legacy names; these Toucan targets now drive 144×168 geometry, not the 160×68 nice!view panel. Verify upright text, all four display edges, and stable idle output on hardware before marking this milestone complete.
