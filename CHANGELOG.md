# Changelog

## [Unreleased]

### Added

- Independent USB reset controller for a third XIAO, with two open-drain reset channels, serial commands, and a wiring diagram.
- ST7789 display rendering and Prospector display wiring support.
- Bluetooth links for a single Paintbrush through Prospector, two Paintbrush halves, and a two-half Prospector receiver, with persistent peer pairing and stale-chord cancellation.
- A dedicated firmware image that clears saved Bluetooth pairing records.
- A portrait nice!view renderer and serial VCOM maintenance for Toucan display integration.
- Automated host tests and nRF52840 firmware builds on pushes and pull requests to `master`.
- Manual versioned releases with changelog notes, tagged source, UF2 firmware, source/example bundles, and checksums.
