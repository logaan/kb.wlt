#!/usr/bin/env bash
set -euo pipefail
milestone_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
exec "$milestone_dir/../../scripts/flash-uf2.sh" "$milestone_dir/../../dist/xiao-led.uf2" "${1:?Pass the mounted XIAO bootloader volume}"
