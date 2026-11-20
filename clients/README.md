# WebAssembly clients

All Wavelet client sources, generated components, and their build tooling live here. Board-specific firmware consumes these clients without duplicating their sources in the board directories.

| Path | Purpose |
| --- | --- |
| `wit/firmware.wit` | Keyboard, braille, and display contracts |
| `build/` | `kb-component-build`: Compilation, validation, canonical adapters, and core extraction |
| `session/` | Shared Rust application state for braille input, navigation, and bindings |
| `components/toucan-keymap/` | Toucan conventional keymap |
| `components/braille/` | Shared Wavelet braille client |
| `components/display/` | Shared Wavelet scenes and menu navigation |
| `components/shift/`, `components/passthrough/` | Reusable keyboard middleware test fixtures |
| `components/generated/` | Checked-in component/core artifacts used by tests and release builds |

## Editing and building

Install the current Wavelet CLI, Binaryen, and WABT 1.0.42. The build recognizes `WAVELET`, `WASM_OPT`, and `WASM2C` as executable overrides. Run commands from the repository root. [Wavelet CLI](https://github.com/logaan/wavelet)

```sh
cargo xtask build toucan-left-satellite-display
cargo xtask build prospector-pair
```

These commands compile their canonical source into `target/components/`, run the native resource probe, and build firmware. The Toucan guest combines keyboard, braille, and display exports. The Paintbrush receiver guest combines braille and display exports. `--source path/to/client.wlt` overrides the source for a one-off build without changing release defaults.

Regenerate all checked-in components after changing client sources:

```sh
./scripts/build-components.sh
cargo xtask check
```

Binaryen optimizes production firmware cores and middleware test fixtures. These fixtures exercise the reusable host pipeline; there are no composed-firmware build targets. Commit both the source and generated artifacts. To check reproducibility without modifying them, pass an empty output directory to `build-components.sh` and compare it with `clients/components/generated/`.

For a single client or an experiment:

```sh
cargo run -p kb-component-build -- firmware keyboard clients/components/toucan-keymap/toucan-keymap.wlt target/client-example
cargo run -p kb-component-build -- optimized keyboard clients/components/shift/shift.wlt target/client-shift
```

Keyboard components thread state and key events through `process`, then actions through `transform-output`. The host validates capacities and converts invalid output or traps into a contained fault rather than continuing with stale key state. WIT contract changes require coordinated updates to this builder and the [Rust host](../host/).
