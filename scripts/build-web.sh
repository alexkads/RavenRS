#!/usr/bin/env bash
# Builds an example for the browser into examples/<name>/web/pkg.
# Usage: scripts/build-web.sh [example-name]   (default: hello-world)
# Requires: rustup target add wasm32-unknown-unknown
#           cargo install wasm-bindgen-cli --version 0.2.130   (must match Cargo.toml)
# Optional: wasm-opt (binaryen) to shrink the output further.
set -euo pipefail

name="${1:-hello-world}"
crate="${name//-/_}"
root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/examples/$name/web/pkg"

cd "$root"
cargo build -p "$name" --lib --target wasm32-unknown-unknown --profile wasm-release
wasm-bindgen --target web --no-typescript --out-dir "$out" \
  "target/wasm32-unknown-unknown/wasm-release/$crate.wasm"

if command -v wasm-opt >/dev/null 2>&1; then
  # The feature list matches what rustc enables by default for wasm32-unknown-unknown;
  # --all-features would emit encodings browsers do not accept yet.
  wasm-opt -Oz --enable-bulk-memory --enable-mutable-globals --enable-multivalue \
    --enable-nontrapping-float-to-int --enable-reference-types --enable-sign-ext \
    -o "$out/${crate}_bg.wasm" "$out/${crate}_bg.wasm"
fi

size=$(wc -c < "$out/${crate}_bg.wasm" | tr -d ' ')
gz=$(gzip -9 -c "$out/${crate}_bg.wasm" | wc -c | tr -d ' ')
echo "Built $out (${size} bytes, ${gz} bytes gzipped)"
echo "Serve it with: python3 -m http.server -d examples/$name/web 8080"
