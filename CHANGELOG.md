# Changelog

## [Unreleased]

### Added

- Optional two-component Toucan firmware without displays, with optimized Wavelet middleware, a Cortex-M heap probe, and a native RAM reserve check before UF2 generation.

- A separate atomic flash journal for keyboard preferences, asynchronous saves, and menu-safe Bluetooth link restart and pairing reset commands.

- Build-time component heap probes, a shared firmware target catalog, and hardware checklists for all ten milestones, including a two-component LED example.

- Prospector braille reference mode enumerates the active Wavelet module, renders six- and eight-dot cells with separate Space labels, and captures namespace, browsing, and lookup input without sending it to the host.

- The Prospector QuickBrailleFox menu provides reference and diagnostics screens, Bluetooth controls, a Computer 8 and UEB Grade 1 switch, and persistent chord and contraction editing.

- Toucan 2 Bluetooth firmware with a Wavelet keymap, held-key reports, momentary layers, nice!view output, and a two-half Prospector USB receiver.
- Independent USB reset controller for a third XIAO, with two open-drain reset channels, serial commands, and a wiring diagram.
- ST7789 display rendering and Prospector display wiring support.
- Bluetooth links for a single Paintbrush through Prospector, two Paintbrush halves, and a two-half Prospector receiver, with persistent peer pairing and stale-chord cancellation.
- A dedicated firmware image that clears saved Bluetooth pairing records.
- A portrait nice!view renderer and serial VCOM maintenance for Toucan display integration.
- Display status for conventional keyboard key counts and active layers alongside HID output.
- Automated host tests and nRF52840 firmware builds on pushes and pull requests to `master`.
- Manual versioned releases with changelog notes, tagged source, UF2 firmware, source/example bundles, and checksums.

### Changed

- The UF2 flashing script reports validation steps and verbose copy output before confirming completion.

- Redesigned the Toucan MIP status screen for 144×168 pixels with a full-width header, link indicator, larger key and layer values, and a separate HID report section.

- Split Paintbrush layouts now assign all eight braille dots to the top rows and braille Space to each inner bottom key.

### Fixed

- Prospector LCD backlight now uses active-high control instead of turning off after initialization, fixing a dark screen while keyboard input still works.

- Prospector dongles declare 500 ppm Bluetooth sleep-clock accuracy to accommodate measured clock drift and avoid split-link handshake timeouts, including the display-free Toucan dongle.

- Corrected the upgraded Toucan 1 display from 160×68 nice!view geometry to its original 144×168 MIP panel, and documented verified Toucan and Prospector display specifications.

- Paintbrush supports one-shot Shift, Control, Alt, Command, and Globe chords. Armed modifiers apply to the next key and expire after one second.

- Prospector displays dot 8 and Space separately in the live braille status view.

- Toucan keeps a shared HID key pressed until all physical keys that map to it have been released.

- Toucan discards expired key snapshots after USB or Bluetooth report backpressure and requires neutral keys when a split link resumes.

- Toucan 2 retains its left nice!view display when Prospector handles the keymap and USB. The display receives split link, key, layer, HID, and fault status over Bluetooth.

- Split keyboard link loss discards pending contractions and HID reports even when the disconnected keyboard has released its keys.

- USB suspend, reset, and disconnect cancel pending keyboard report writes. The next USB session starts with a neutral report.

- Interrupted Bluetooth saves preserve newly learned pairings in memory, and clearing saved pairings commits an empty journal record before erasing the previous page.

- Preferences retry transient flash failures, and unchanged Bluetooth subscriptions retry a failed pairing save.

- Bonded Bluetooth notification subscriptions survive firmware restarts and remain isolated by peer identity and server schema.

- Pairing flash failures preserve active keyboards and volatile bonds, with a separate warning on displays and a distinct blink pattern on headless boards.

- Firmware and resetter builds apply their Cortex-M linker script even when invoked from the repository root with `--manifest-path`.

- Displays distinguish unsaved Bluetooth pairing records from connection state and fatal component errors.

- Reference mapping discovery yields to USB and Bluetooth tasks while loading custom modules.

- USB and Bluetooth HID descriptors cover the full encoded keyboard usage range, including F13–F24 from custom Wavelet keymaps.

- Paintbrush component traps release typing state and keep USB or Bluetooth alive; failed keyboard and LED components signal a latched fault until firmware is replaced.

- Toucan link loss cancels pending middleware actions, and component failures release all held keys while Bluetooth, USB, and fault displays remain active.

- Prospector component failures stop typing, release pending HID state, preserve USB and radio tasks, and display a persistent firmware fault instead of halting the device.

- Pairing records reject truncated, trailing, or unsupported payloads and fall back to the previous valid flash journal entry.

- Split links reconnect after stalled GATT discovery or missing key heartbeats, and Bluetooth advertising failures retry without halting firmware.

- Bluetooth HID suspend and protocol changes cancel pending chords, reconnect starts with released keys, and saved host pairings are restored together.

- USB suspend, reset, and reconnect cancel in-progress chords and discard reports from earlier connection sessions.

- USB keyboards accept boot protocol, LED output reports, and host idle requests, and return their current HID report.
