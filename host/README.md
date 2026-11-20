# Rust component host

`runtime/` implements the WIT canonical ABI for keyboard, braille, and display guests. Native tests, resource probes, and middleware fixtures use Wasmi. Cortex-M firmware translates the validated production core Wasm ahead of time with WABT 1.0.42 (`wasm2c`) and links the result into the image. The embedded runtime provides a bounded 64 KiB guest memory and meters guest execution.

Each application board runs one instance. The Toucan left half runs keyboard, braille, and display together. Its Prospector renders the relayed display model with its own instance. The Paintbrush Prospector handles braille and display. Key-scanning satellites forward matrix snapshots.

From the repository root:

```sh
cargo test -p kb-component-runtime --locked
cargo check -p kb-component-runtime --target thumbv7em-none-eabihf --locked
cargo xtask probe keyboard clients/components/generated/toucan-keymap/keyboard.core.wasm
cargo xtask probe braille clients/components/generated/braille.core.wasm
```

Tests and probes cover ABI validation, malformed outputs, traps, chaining, resource use, and typing behavior. Normal builds check the selected artifact before cross-building and require at least 16 KiB of native RAM reserve before UF2 generation. Check the board READMEs for hardware validation.
