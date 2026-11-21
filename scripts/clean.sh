#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
rm -rf -- "$repo_dir/target" "$repo_dir/firmware/nrf52840/target" "$repo_dir/emulator/dist"
dist="$repo_dir/dist"
if [[ -d "$dist" && ! -L "$dist" ]]; then
  find "$dist" -maxdepth 1 -type f \( -name '*.uf2' -o -name '*.elf' -o -name '*.component.wasm' \) -delete
  if [[ -d "$dist/release" ]]; then
    find "$dist/release" -maxdepth 1 -type f \( -name '*.uf2' -o -name SHA256SUMS \) -delete
  fi
  if [[ -d "$dist/packages" ]]; then
    find "$dist/packages" -maxdepth 1 -type f \( -name 'kb.wlt-toucan-*.zip' -o -name 'kb.wlt-paintbrush-*.zip' -o -name SHA256SUMS \) -delete
  fi
fi
printf 'Removed local build artifacts.\n'
