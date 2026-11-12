#!/usr/bin/env bash
set -euo pipefail
if [[ $# != 2 ]]; then
  printf 'Usage: %s <left-xiao-volume> <right-xiao-volume>\n' "$0" >&2
  exit 2
fi
milestone_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_dir=$(cd -- "$milestone_dir/../.." && pwd)
"$repo_dir/scripts/flash-uf2.sh" "$repo_dir/dist/toucan-ble-composed.uf2" "$1"
"$repo_dir/scripts/flash-uf2.sh" "$repo_dir/dist/toucan-right-satellite.uf2" "$2"
