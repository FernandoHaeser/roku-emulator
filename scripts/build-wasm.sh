#!/usr/bin/env bash
# Builds the BrightScript engine to WebAssembly and copies it into the web app.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
./scripts/cargo.sh build -p brs-wasm --target wasm32-unknown-unknown --release
mkdir -p apps/web/src/wasm
cp target/wasm32-unknown-unknown/release/brs_wasm.wasm apps/web/src/wasm/brs.wasm
echo "wasm: apps/web/src/wasm/brs.wasm ($(wc -c < apps/web/src/wasm/brs.wasm | tr -d ' ') bytes)"
