#!/usr/bin/env bash
set -euo pipefail
milestone_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
exec "$milestone_dir/../../scripts/flash-uf2.sh" "$milestone_dir/../../dist/paintbrush-usb.uf2" "${1:?Pass the mounted nice!nano bootloader volume}"
