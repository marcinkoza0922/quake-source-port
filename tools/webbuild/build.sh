#!/usr/bin/env bash
# Build the browser (wasm32 + WebGPU) version into target/web/.
# Serve with: python3 -m http.server -d target/web
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
target_dir="${CARGO_TARGET_DIR:-$root/target}"
cargo rustc --offline --release --lib --crate-type cdylib \
  --target wasm32-unknown-unknown -p quake-source-port
bindgen="${WASM_BINDGEN:-$HOME/.cargo/bin/wasm-bindgen}"
out="$root/target/web"
mkdir -p "$out"
"$bindgen" --target web --no-typescript --out-dir "$out" \
  "$target_dir/wasm32-unknown-unknown/release/quake_source_port.wasm"
cp web/* "$out/"
echo "web build ready: $out (python3 -m http.server -d $out)"
