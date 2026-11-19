#!/usr/bin/env bash
set -euo pipefail
if [[ $# != 2 ]]; then
  printf 'Usage: %s <firmware.uf2> <mounted-bootloader-volume>\n' "$0" >&2
  exit 2
fi
image=$1
volume=$2
printf 'Checking UF2 image: %s\n' "$image"
printf 'Checking bootloader volume: %s\n' "$volume"
if [[ ! -f "$image" || ! -d "$volume" || ! -f "$volume/INFO_UF2.TXT" ]]; then
  printf 'Expected an existing UF2 image and a mounted UF2 volume with INFO_UF2.TXT.\n' >&2
  exit 1
fi
printf 'Checking board identification in %s/INFO_UF2.TXT...\n' "$volume"
if ! grep -Eqi 'nrf52840|nice.?nano|xiao' "$volume/INFO_UF2.TXT"; then
  printf 'The selected UF2 volume does not identify an nRF52840 board.\n' >&2
  exit 1
fi
printf 'Board identified as nRF52840. Copying firmware; this may take a moment...\n'
copied=true
COPYFILE_DISABLE=1 cp -v "$image" "$volume/$(basename "$image")" || copied=false
for ((i=0; i<30; i++)); do
  if [[ ! -f "$volume/INFO_UF2.TXT" ]]; then
    printf 'Bootloader disconnected. Verify that the board starts in its normal role.\n'
    exit 0
  fi
  sleep 1
done
printf 'Bootloader still mounted (copy completed: %s). Flash not confirmed. Re-enter boot mode before retrying.\n' "$copied" >&2
exit 1
