#!/usr/bin/env bash
set -euo pipefail
milestone_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
cd "$milestone_dir/../.."
cargo xtask build xiao-led --source "$milestone_dir/led.wlt" "$@"
