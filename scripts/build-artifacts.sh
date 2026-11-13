#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
output_dir=${1:-"$repo_dir/dist/release"}
firmware_target_dir=${KB_FIRMWARE_TARGET_DIR:-"$repo_dir/firmware/nrf52840/target"}
mkdir -p "$output_dir"
output_dir=$(cd -- "$output_dir" && pwd)
cd "$repo_dir"
cargo build --locked -p xtask
"$repo_dir/target/debug/xtask" probe led clients/components/generated/led.core.wasm
"$repo_dir/target/debug/xtask" probe led clients/components/generated/led-dim/stage0/led.core.wasm clients/components/generated/led-dim/stage1/led.core.wasm
"$repo_dir/target/debug/xtask" probe braille clients/components/generated/braille.core.wasm
"$repo_dir/target/debug/xtask" probe keyboard clients/components/generated/toucan-keymap/keyboard.core.wasm
"$repo_dir/target/debug/xtask" probe-arm
target_list=$(mktemp)
trap 'rm -f "$target_list"' EXIT
"$repo_dir/target/debug/xtask" targets > "$target_list"
while IFS='|' read -r artifact_name binary features origin converter_option; do
  (
    cd "$repo_dir/firmware/nrf52840"
    KB_FLASH_ORIGIN="$origin" CARGO_TARGET_DIR="$firmware_target_dir" cargo build --locked --release --no-default-features --features "$features" --bin "$binary"
  )
  set --
  if [[ -n "$converter_option" ]]; then
    set -- "$converter_option"
  fi
  "$repo_dir/target/debug/xtask" uf2 \
    "$firmware_target_dir/thumbv7em-none-eabihf/release/$binary" \
    "$output_dir/$artifact_name.uf2" "$@"
done < "$target_list"
(
  cd "$repo_dir/resetter"
  KB_FLASH_ORIGIN=0x27000 cargo build --locked --release --features firmware --bin kb-resetter
)
"$repo_dir/target/debug/xtask" uf2 \
  "$repo_dir/resetter/target/thumbv7em-none-eabihf/release/kb-resetter" \
  "$output_dir/resetter.uf2" --s140v7
