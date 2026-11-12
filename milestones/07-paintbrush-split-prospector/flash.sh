#!/usr/bin/env bash
set -euo pipefail
milestone_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
cd "$milestone_dir/../.."
if [[ $# != 3 ]]; then
  printf 'Pass bootloader volume paths in this order: paintbrush-left-satellite, paintbrush-right-satellite, prospector-pair\n' >&2
  exit 2
fi
"$milestone_dir/../../scripts/flash-uf2.sh" "$milestone_dir/../../dist/paintbrush-left-satellite.uf2" "$1"
"$milestone_dir/../../scripts/flash-uf2.sh" "$milestone_dir/../../dist/paintbrush-right-satellite.uf2" "$2"
"$milestone_dir/../../scripts/flash-uf2.sh" "$milestone_dir/../../dist/prospector-pair.uf2" "$3"
