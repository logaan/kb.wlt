#!/usr/bin/env bash
set -euo pipefail
milestone_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
cd "$milestone_dir/../.."
cargo xtask build paintbrush-left-satellite --source "$milestone_dir/braille.wlt"
cargo xtask build paintbrush-right-satellite --source "$milestone_dir/braille.wlt"
cargo xtask build prospector-pair --source "$milestone_dir/braille.wlt"
