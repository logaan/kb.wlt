#!/usr/bin/env bash
set -euo pipefail
configure() {
  local compiler root lib resource
  for compiler in /usr/lib/llvm-*/bin/clang; do
    [[ -x "$compiler" ]] || continue
    root=${compiler%/bin/clang}
    lib=$(find "$root/lib" -maxdepth 1 \( -name libclang.so -o -name 'libclang.so.*' \) -print -quit)
    resource=$("$compiler" -print-resource-dir)
    [[ -f "$lib" && -f "$resource/include/stdint.h" ]] || continue
    printf 'CLANG_PATH=%s\nLIBCLANG_PATH=%s\nBINDGEN_EXTRA_CLANG_ARGS_thumbv7em_none_eabihf=-ffreestanding -isystem %s/include\n' "$compiler" "$lib" "$resource" >> "$GITHUB_ENV"
    return 0
  done
  return 1
}
if ! configure; then
  sources="$RUNNER_TEMP/kb-ubuntu.list"
  printf '%s\n' 'deb [signed-by=/usr/share/keyrings/ubuntu-archive-keyring.gpg] https://archive.ubuntu.com/ubuntu noble main universe' 'deb [signed-by=/usr/share/keyrings/ubuntu-archive-keyring.gpg] https://archive.ubuntu.com/ubuntu noble-updates main universe' 'deb [signed-by=/usr/share/keyrings/ubuntu-archive-keyring.gpg] https://security.ubuntu.com/ubuntu noble-security main universe' > "$sources"
  options=(-o "Dir::Etc::sourcelist=$sources" -o Dir::Etc::sourceparts=-)
  sudo apt-get "${options[@]}" update
  sudo apt-get "${options[@]}" install -y clang libclang-dev
  configure
fi
