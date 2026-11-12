# Changelog

## [Unreleased]

### Added

- Build-time component heap probes, a shared firmware target catalog, and hardware checklists for all ten milestones, including a two-component LED example.

- Prospector braille reference mode enumerates the active Wavelet module, renders six- and eight-dot cells with separate Space labels, and captures namespace, browsing, and lookup input without sending it to the host.

- Toucan 2 Bluetooth firmware with a Wavelet keymap, held-key reports, momentary layers, nice!view output, and a two-half Prospector USB receiver.
- Independent USB reset controller for a third XIAO, with two open-drain reset channels, serial commands, and a wiring diagram.
- ST7789 display rendering and Prospector display wiring support.
- Bluetooth links for a single Paintbrush through Prospector, two Paintbrush halves, and a two-half Prospector receiver, with persistent peer pairing and stale-chord cancellation.
- A dedicated firmware image that clears saved Bluetooth pairing records.
- A portrait nice!view renderer and serial VCOM maintenance for Toucan display integration.
- Display status for conventional keyboard key counts and active layers alongside HID output.
- Automated host tests and nRF52840 firmware builds on pushes and pull requests to `master`.
- Manual versioned releases with changelog notes, tagged source, UF2 firmware, source/example bundles, and checksums.

### Fixed

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
