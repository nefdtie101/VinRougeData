#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "==> Building WASM frontend..."
cd "$SCRIPT_DIR/vinrouge-web"
trunk build

echo "==> Starting desktop dev server..."
cd "$SCRIPT_DIR/vinrouge-desktop"
cargo tauri dev
