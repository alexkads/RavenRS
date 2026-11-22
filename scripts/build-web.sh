#!/usr/bin/env bash
# Builds an example (or a tool, such as the feature center) for the browser into
# examples/<name>/web/pkg or tools/<name>/web/pkg.
# Usage: scripts/build-web.sh [example-name] [cargo flags...]   (default: hello-world)
#        scripts/build-web.sh raven-ci   (a tool whose web/crate names the package that compiles
#                                         for the browser: raven-ci-painel, out in tools/raven-ci/web/pkg)
#        scripts/build-web.sh raven-feature-center --no-default-features --features widgets,playground
# Requires: rustup target add wasm32-unknown-unknown
#           cargo install wasm-bindgen-cli --version 0.2.130   (must match Cargo.toml)
# Optional: wasm-opt (binaryen) to shrink the output further.
set -euo pipefail

name="${1:-hello-world}"
shift || true
crate="${name//-/_}"
root="$(cd "$(dirname "$0")/.." && pwd)"
folder="examples"
if [ -d "$root/tools/$name/web" ]; then folder="tools"; fi
out="$root/$folder/$name/web/pkg"
# A tool whose own package does not compile for the browser (a server) names, in web/crate, the
# package that does.
package="$name"
if [ -f "$root/$folder/$name/web/crate" ]; then
  package="$(tr -d '[:space:]' < "$root/$folder/$name/web/crate")"
  crate="${package//-/_}"
fi

cd "$root"
cargo build -p "$package" --lib --target wasm32-unknown-unknown --profile wasm-release "$@"
wasm-bindgen --target web --no-typescript --out-dir "$out" \
  "${CARGO_TARGET_DIR:-target}/wasm32-unknown-unknown/wasm-release/$crate.wasm"

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
echo "Serve it with: python3 -m http.server -d $folder/$name/web 8080"
