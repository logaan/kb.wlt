#!/usr/bin/env bash
set -euo pipefail
lib=$(find /usr/lib/llvm-*/lib -maxdepth 1 \( -name libclang.so -o -name 'libclang.so.*' \) -print -quit 2>/dev/null || true)
if command -v clang >/dev/null && [[ -f "$lib" ]]; then
  printf 'LIBCLANG_PATH=%s\n' "$lib" >> "$GITHUB_ENV"
else
  sources="$RUNNER_TEMP/kb-ubuntu.list"
  printf '%s\n' 'deb [signed-by=/usr/share/keyrings/ubuntu-archive-keyring.gpg] https://archive.ubuntu.com/ubuntu noble main universe' 'deb [signed-by=/usr/share/keyrings/ubuntu-archive-keyring.gpg] https://archive.ubuntu.com/ubuntu noble-updates main universe' 'deb [signed-by=/usr/share/keyrings/ubuntu-archive-keyring.gpg] https://security.ubuntu.com/ubuntu noble-security main universe' > "$sources"
  options=(-o "Dir::Etc::sourcelist=$sources" -o Dir::Etc::sourceparts=-)
  sudo apt-get "${options[@]}" update
  sudo apt-get "${options[@]}" install -y clang libclang-dev
fi
