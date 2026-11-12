#!/usr/bin/env bash
set -euo pipefail
milestone_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
cd "$milestone_dir/../.."
cargo xtask build toucan-ble-composed --source "$milestone_dir/keymap.wlt" --middleware "$milestone_dir/shift.wlt"
cargo xtask build toucan-right-satellite --source "$milestone_dir/keymap.wlt"
