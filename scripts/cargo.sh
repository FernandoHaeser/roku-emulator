#!/usr/bin/env bash
# Runs cargo from the rustup toolchain pinned by rust-toolchain.toml (which
# includes the wasm32 target), even when a Homebrew `cargo`/`rustc` without
# wasm support comes first on PATH.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

rustup_bin=""
if command -v rustup >/dev/null 2>&1; then
  rustup_bin="rustup"
elif [ -x /opt/homebrew/opt/rustup/bin/rustup ]; then
  rustup_bin="/opt/homebrew/opt/rustup/bin/rustup"
fi

if [ -n "$rustup_bin" ]; then
  toolchain_bin="$(dirname "$("$rustup_bin" which rustc)")"
  export PATH="$toolchain_bin:$PATH"
fi
exec cargo "$@"
