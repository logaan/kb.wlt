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
wabt="$RUNNER_TEMP/wabt"
archive="$RUNNER_TEMP/wabt-1.0.42-linux-x64.tar.gz"
mkdir -p "$wabt"
curl -fsSL https://github.com/WebAssembly/wabt/releases/download/1.0.42/wabt-1.0.42-linux-x64.tar.gz -o "$archive"
printf '84895407a6bbb80e918f33b16b2fb2206021c150b6bc9ff6f761263a745ab131  %s\n' "$archive" | sha256sum -c
tar -xzf "$archive" -C "$wabt" --strip-components=1
"$wabt/bin/wasm2c" --version
printf '%s\n' "$wabt/bin" >> "$GITHUB_PATH"
