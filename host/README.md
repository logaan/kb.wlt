# Rust component host

`runtime/` is the `kb-component-runtime` crate. It runs validated Wasm core modules with Wasmi, implements the WIT canonical ABI adapters, and exposes braille and keyboard components plus reusable input/output middleware pipelines. It is `no_std` with allocation; firmware provides the heap. Interface definitions and component compilation belong to [clients/](../clients/).

Runtime tests cover ABI validation, malformed outputs, traps, chaining, heap budgets, and typing behavior. Board-specific keymap tests live under `toucan/tests/` and are included by this crate's manifest. The LED contract/runtime and standalone Cortex-M example project have been removed.

From the repository root:

```sh
cargo test -p kb-component-runtime --locked
cargo check -p kb-component-runtime --target thumbv7em-none-eabihf --locked
cargo xtask probe keyboard clients/components/generated/toucan-keymap/keyboard.core.wasm
cargo xtask probe braille clients/components/generated/braille.core.wasm
```

Normal builds probe component heap use before cross-building and enforce at least 16 KiB of native RAM reserve before UF2 generation. The remaining firmware uses a 160 KiB component heap. Native heap measurements and cross-build checks are not substitutes for hardware testing.

The host retains generic middleware APIs and their test fixtures for future development. They do not add firmware variants or release images.
