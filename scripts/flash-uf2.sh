#!/usr/bin/env bash
set -euo pipefail
if [[ $# != 2 ]]; then
  printf 'Usage: %s <firmware.uf2> <mounted-bootloader-volume>\n' "$0" >&2
  exit 2
fi
image=$1
volume=$2
if [[ ! -f "$image" || ! -d "$volume" || ! -f "$volume/INFO_UF2.TXT" ]]; then
  printf 'Expected an existing UF2 image and a mounted UF2 volume with INFO_UF2.TXT.\n' >&2
  exit 1
fi
if ! grep -Eqi 'nrf52840|nice.?nano|xiao' "$volume/INFO_UF2.TXT"; then
  printf 'The selected UF2 volume does not identify an nRF52840 board.\n' >&2
  exit 1
fi
cp "$image" "$volume/$(basename "$image")"
printf 'Copied %s to %s. The board should reboot.\n' "$image" "$volume"
