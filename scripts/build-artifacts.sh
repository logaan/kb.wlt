#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
output_dir=${1:-"$repo_dir/dist/release"}
firmware_target_dir=${KB_FIRMWARE_TARGET_DIR:-"$repo_dir/firmware/nrf52840/target"}
mkdir -p "$output_dir"
output_dir=$(cd -- "$output_dir" && pwd)
cd "$repo_dir"
cargo build --locked -p xtask
xtask="$repo_dir/target/debug/xtask"
"$xtask" probe braille --display clients/components/generated/braille.core.wasm
"$xtask" probe keyboard --display clients/components/generated/toucan-keymap/keyboard.core.wasm
staging=$(mktemp -d)
trap 'rm -rf "$staging"' EXIT
"$xtask" targets > "$staging/targets"
while IFS='|' read -r artifact_name binary features origin converter_option; do
  (
    cd "$repo_dir/firmware/nrf52840"
    KB_FLASH_ORIGIN="$origin" CARGO_TARGET_DIR="$firmware_target_dir" cargo build --locked --release --no-default-features --features "$features" --bin "$binary"
  )
  set --
  if [[ -n "$converter_option" ]]; then
    set -- "$converter_option"
  fi
  "$xtask" uf2 \
    "$firmware_target_dir/thumbv7em-none-eabihf/release/$binary" \
    "$staging/$artifact_name.uf2" "$@"
done < "$staging/targets"
"$xtask" verify-release "$staging"
python3 - "$staging" <<'PYHASH'
import hashlib
import sys
from pathlib import Path

directory = Path(sys.argv[1])
checksums = "".join(
    f"{hashlib.sha256(image.read_bytes()).hexdigest()}  {image.name}\n"
    for image in sorted(directory.glob("*.uf2"))
)
(directory / "SHA256SUMS").write_text(checksums, encoding="ascii")
PYHASH
find "$output_dir" -maxdepth 1 -type f -name '*.uf2' -delete
cp "$staging"/*.uf2 "$output_dir/"
mv -f "$staging/SHA256SUMS" "$output_dir/SHA256SUMS"
"$xtask" verify-release "$output_dir"
