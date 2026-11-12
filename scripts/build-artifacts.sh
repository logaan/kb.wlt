#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
output_dir=${1:-"$repo_dir/dist/release"}
firmware_target_dir=${KB_FIRMWARE_TARGET_DIR:-"$repo_dir/firmware/nrf52840/target"}
mkdir -p "$output_dir"
output_dir=$(cd -- "$output_dir" && pwd)
cd "$repo_dir"
cargo build --locked -p xtask
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
done <<'TARGETS'
xiao-led|xiao-led|usb|0x27000|--s140v7
xiao-led-composed|xiao-led|usb,composed|0x27000|--s140v7
paintbrush-usb|paintbrush-usb|usb|0x26000|
paintbrush-usb-left|paintbrush-usb|usb,paintbrush-left|0x26000|
paintbrush-ble|paintbrush-ble|ble|0x26000|
paintbrush-ble-left|paintbrush-ble|ble,paintbrush-left|0x26000|
paintbrush-display|paintbrush-usb|usb,display|0x26000|
paintbrush-ble-display|paintbrush-ble|ble,display|0x26000|
paintbrush-satellite|paintbrush-satellite|split|0x26000|
paintbrush-satellite-left|paintbrush-satellite|split,satellite-left|0x26000|
paintbrush-satellite-right|paintbrush-satellite|split,satellite-right|0x26000|
paintbrush-central|paintbrush-central|usb,split|0x26000|
prospector-single|prospector-dongle|usb,split,prospector-display|0x27000|--s140v7
prospector-pair|prospector-dongle|usb,split,prospector-display,split-pair|0x27000|--s140v7
TARGETS
(
  cd "$repo_dir/resetter"
  KB_FLASH_ORIGIN=0x27000 cargo build --locked --release --features firmware --bin kb-resetter
)
"$repo_dir/target/debug/xtask" uf2 \
  "$repo_dir/resetter/target/thumbv7em-none-eabihf/release/kb-resetter" \
  "$output_dir/resetter.uf2" --s140v7
