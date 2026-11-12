#!/usr/bin/env bash
set -euo pipefail
milestone_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
cd "$milestone_dir/../.."
if [[ $# != 3 ]]; then
  printf 'Pass bootloader volume paths in this order: toucan-left-satellite, toucan-right-satellite, toucan-prospector\n' >&2
  exit 2
fi
"$milestone_dir/../../scripts/flash-uf2.sh" "$milestone_dir/../../dist/toucan-left-satellite.uf2" "$1"
"$milestone_dir/../../scripts/flash-uf2.sh" "$milestone_dir/../../dist/toucan-right-satellite.uf2" "$2"
"$milestone_dir/../../scripts/flash-uf2.sh" "$milestone_dir/../../dist/toucan-prospector.uf2" "$3"
