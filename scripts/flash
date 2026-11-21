#!/usr/bin/env bash
set -euo pipefail
if [[ $# -gt 2 ]]; then
  printf 'Usage: %s [firmware.uf2] [mounted-bootloader-volume]\n' "$0" >&2
  exit 2
fi

choose() {
  local prompt=$1 choice
  shift
  PS3="$prompt (number, or q to quit): "
  select choice in "$@"; do
    if [[ "$REPLY" == q ]]; then
      return 1
    fi
    if [[ -n "$choice" ]]; then
      printf '%s\n' "$choice"
      return 0
    fi
    printf 'Enter a listed number or q to quit.\n' >&2
  done
  return 1
}

shopt -s nullglob
image=${1:-}
volume=${2:-}
if [[ -z "$image" ]]; then
  script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
  dist="${script_dir%/*}/dist"
  images=()
  for candidate in "$script_dir"/*.uf2 "$dist"/*.uf2 "$dist"/release/*.uf2; do
    if [[ -f "$candidate" ]]; then
      images+=("$candidate")
    fi
  done
  if [[ ${#images[@]} -eq 0 ]]; then
    printf 'No firmware images found. Build the firmware or pass a UF2 path.\n' >&2
    exit 1
  fi
  image=$(choose 'Select firmware' "${images[@]}")
fi

printf 'Checking UF2 image: %s\n' "$image"
if [[ ! -f "$image" ]]; then
  printf 'Expected an existing UF2 image.\n' >&2
  exit 1
fi

if [[ -z "$volume" ]]; then
  volumes=()
  for candidate in /Volumes/* /media/* /media/*/* /run/media/*/* /mnt/*; do
    if [[ -f "$candidate/INFO_UF2.TXT" ]]; then
      volumes+=("$candidate")
    fi
  done
  if [[ ${#volumes[@]} -eq 0 ]]; then
    printf 'No mounted UF2 volumes found. Enter the board bootloader or pass a volume path.\n' >&2
    exit 1
  fi
  volume=$(choose 'Select bootloader volume' "${volumes[@]}")
fi

printf 'Checking bootloader volume: %s\n' "$volume"
if [[ ! -f "$volume/INFO_UF2.TXT" ]]; then
  printf 'Expected a mounted UF2 volume with INFO_UF2.TXT.\n' >&2
  exit 1
fi
printf 'Checking board identification in %s/INFO_UF2.TXT...\n' "$volume"
if ! grep -Eqi -- 'nrf52840|nice.?nano|xiao' "$volume/INFO_UF2.TXT"; then
  printf 'The selected UF2 volume does not identify an nRF52840 board.\n' >&2
  exit 1
fi
printf 'Board identified as nRF52840. Copying firmware; this may take a moment...\n'
copied=true
COPYFILE_DISABLE=1 cp -v -- "$image" "$volume/${image##*/}" || copied=false
for ((i=0; i<30; i++)); do
  if [[ ! -f "$volume/INFO_UF2.TXT" ]]; then
    printf 'Bootloader disconnected. Verify that the board starts in its normal role.\n'
    exit 0
  fi
  sleep 1
done
printf 'Bootloader still mounted (copy completed: %s). Flash not confirmed. Re-enter boot mode before retrying.\n' "$copied" >&2
exit 1
