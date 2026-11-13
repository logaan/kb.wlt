# Rust component host

`runtime/` is the `kb-component-runtime` crate. It runs validated Wasm core modules with Wasmi, implements the WIT canonical ABI adapters, and exposes LED, braille, and keyboard components plus input/output middleware pipelines. It is `no_std` with allocation; firmware provides the heap. Interface definitions and component compilation belong to [clients/](../clients/), not the host.

Runtime tests cover ABI validation, malformed outputs, traps, chaining, heap budgets, and typing behavior. Board-specific keymap tests live under `toucan/tests/` and are included by this crate's manifest.

From the repository root:

```sh
cargo test -p kb-component-runtime --locked
cargo check -p kb-component-runtime --target thumbv7em-none-eabihf --locked
cargo xtask probe keyboard clients/components/generated/toucan-keymap/keyboard.core.wasm
```

`heap-probe/` is a separate Cortex-M firmware workspace. It measures the composed keyboard pipeline under QEMU with the same 196 KiB allocator budget used by display-free composed Toucan firmware:

```sh
cargo xtask probe-arm
```

Install `qemu-system-arm` first, or set `QEMU_SYSTEM_ARM` to its executable. Normal `cargo xtask build` commands run the appropriate component probe before cross-building and enforce at least 16 KiB of native RAM reserve before UF2 generation. Heap and cross-build checks are not substitutes for hardware testing.
