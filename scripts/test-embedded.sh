#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
probe="$repo_dir/host/runtime/tests/embedded"
target_dir="$repo_dir/target/embedded-probe"
for role in braille keyboard; do
  set --
  if [[ "$role" == keyboard ]]; then
    set -- --features keyboard
  fi
  CARGO_TARGET_DIR="$target_dir" cargo build     --manifest-path "$probe/Cargo.toml" --locked --release     --target thumbv7em-none-eabihf "$@"
  "${QEMU_SYSTEM_ARM:-qemu-system-arm}"     -M mps2-an386 -nographic -semihosting-config enable=on,target=native     -kernel "$target_dir/thumbv7em-none-eabihf/release/kb-embedded-runtime-probe"
done
