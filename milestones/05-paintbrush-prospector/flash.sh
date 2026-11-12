#!/usr/bin/env bash
set -euo pipefail
milestone_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
cd "$milestone_dir/../.."
if [[ $# != 2 ]]; then
  printf 'Pass bootloader volume paths in this order: paintbrush-satellite, prospector-single\n' >&2
  exit 2
fi
"$milestone_dir/../../scripts/flash-uf2.sh" "$milestone_dir/../../dist/paintbrush-satellite.uf2" "$1"
"$milestone_dir/../../scripts/flash-uf2.sh" "$milestone_dir/../../dist/prospector-single.uf2" "$2"
