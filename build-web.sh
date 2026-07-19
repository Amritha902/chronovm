#!/usr/bin/env bash
# Build the chronovm browser debugger.
#
# Compiles the VM core to wasm and generates the JS bindings into docs/pkg/
# (docs/ is what GitHub Pages serves).
# Requires: the wasm32 target and wasm-bindgen-cli (matching the wasm-bindgen
# crate version pinned in Cargo.toml).
#
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli
#
# Then serve the docs/ directory over HTTP (ES modules + wasm need http, not
# file://):
#
#   ./build-web.sh && (cd docs && python3 -m http.server 8080)
#   open http://localhost:8080
set -euo pipefail
cd "$(dirname "$0")"

echo "› compiling core to wasm32…"
cargo build --release --lib --no-default-features --features wasm \
  --target wasm32-unknown-unknown

echo "› generating JS bindings…"
wasm-bindgen --target web --out-dir docs/pkg --no-typescript \
  target/wasm32-unknown-unknown/release/chronovm.wasm

echo "✓ built. serve it with:  (cd docs && python3 -m http.server 8080)"
