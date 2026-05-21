#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CORE_DIR="$SCRIPT_DIR/core"
OUT_DIR="$SCRIPT_DIR/web/src/wasm"
WASM_FILE="$CORE_DIR/target/wasm32-unknown-unknown/release/texsch.wasm"

echo "==> Step 1/2: cargo build --target wasm32-unknown-unknown --release"
cd "$CORE_DIR"
cargo build --target wasm32-unknown-unknown --release
echo "    Done: $WASM_FILE"

echo "==> Step 2/2: wasm-bindgen"
mkdir -p "$OUT_DIR"
wasm-bindgen \
    --target web \
    --out-dir "$OUT_DIR" \
    "$WASM_FILE"
echo "    Done: $(ls "$OUT_DIR")"

echo ""
echo "WASM build complete. Output in web/src/wasm/"
