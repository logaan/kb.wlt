# Shared Rust support

These workspace crates contain code shared by keyboard applications and the component host:

- `keyboard-core/` (`kb-core`): HID reports, key-state tracking, chords, braille typing primitives, preferences, menu/reference models, and transport/storage protocols.
- `display/` (`kb-display`): display status, ST7789 controller support, braille menus/reference rendering, and panel bus traits.
- `bond-storage/` (`kb-bond-storage`): persistent Bluetooth bonding and preference storage.

Toucan-specific matrix and display modules and their tests live in [toucan/](../toucan/). The relevant shared manifests/modules include them explicitly to preserve the existing Rust APIs without duplicating code.

Run tests and cross-check the portable crates from the repository root:

```sh
cargo test -p kb-core -p kb-display -p kb-bond-storage --locked
cargo check -p kb-core -p kb-display --target thumbv7em-none-eabihf --locked
```

Hardware access and application task orchestration belong to the board folders and [firmware/](../firmware/), not these portable crates.
