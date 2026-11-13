# WebAssembly clients

All Wavelet client sources, generated components, and their build tooling live here. Board-specific firmware consumes these clients without duplicating their sources in the board directories.

| Path | Purpose |
| --- | --- |
| `wit/firmware.wit` | LED, braille, and keyboard contracts |
| `build/` | `kb-component-build`: compilation, validation, canonical adapters, and core extraction |
| `components/toucan-keymap/` | Default tested two-layer Toucan bindings; braille placeholders remain inactive |
| `components/braille/` | Paintbrush braille client |
| `components/led/`, `components/led-dim/` | LED application and composed dimming example |
| `components/shift/`, `components/passthrough/` | Keyboard middleware examples |
| `components/generated/` | Checked-in component/core artifacts used by tests and release builds |

## Editing and building

Install the current [Wavelet CLI](https://github.com/logaan/wavelet). `WAVELET` can select an alternative executable. Run commands from the repository root:

```sh
cargo xtask build toucan-prospector
cargo xtask build prospector-single
cargo xtask build xiao-led
```

These commands compile their canonical source into `target/components/`, probe its heap use, and build firmware. `--source path/to/client.wlt` overrides the source for a one-off build without changing release defaults.

Regenerate all checked-in components after changing client sources:

```sh
./scripts/build-components.sh
cargo test --workspace --locked
cargo xtask probe-arm
```

This requires Binaryen (`wasm-opt`, or `WASM_OPT`) for optimized keyboard stages and QEMU for the ARM probe. Commit both the source and generated artifacts. To check reproducibility without modifying them, pass an empty output directory to `build-components.sh` and compare it with `clients/components/generated/`.

For a single client or an experiment:

```sh
cargo run -p kb-component-build -- keyboard clients/components/toucan-keymap/toucan-keymap.wlt target/client-example
cargo run -p kb-component-build -- optimized keyboard clients/components/shift/shift.wlt target/client-shift
```

Keyboard components thread state and key events through `process`, then actions through `transform-output`. The host validates capacities and converts invalid output or traps into a contained fault rather than continuing with stale key state. WIT contract changes require coordinated updates to this builder and the [Rust host](../host/).
