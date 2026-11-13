#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_dir"
output_dir=${1:-"$repo_dir/clients/components/generated"}
mkdir -p "$output_dir"
output_dir=$(cd -- "$output_dir" && pwd)
cargo build --locked -p kb-component-build
builder="$repo_dir/target/debug/kb-component-build"
"$builder" led clients/components/led/led.wlt "$output_dir"
"$builder" braille clients/components/braille/braille.wlt "$output_dir"
"$builder" keyboard clients/components/toucan-keymap/toucan-keymap.wlt "$output_dir/toucan-keymap"
"$builder" keyboard clients/components/passthrough/passthrough.wlt "$output_dir/passthrough"
"$builder" keyboard clients/components/shift/shift.wlt "$output_dir/shift"
"$builder" chain led clients/components/led-dim/led-dim.wlt clients/components/led/led.wlt "$output_dir/led-dim"
"$builder" optimized keyboard clients/components/shift/shift.wlt "$output_dir/keyboard-composed/stage0"
"$builder" optimized keyboard clients/components/toucan-keymap/toucan-keymap.wlt "$output_dir/keyboard-composed/stage1"
