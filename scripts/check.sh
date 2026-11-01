#!/usr/bin/env bash
# Runs every check locally: formatting, lints (native and wasm32), tests and the web build.
# The project does not use hosted CI; run this before pushing.
set -euo pipefail
cd "$(dirname "$0")/.."

step() { printf '\n\033[1m==> %s\033[0m\n' "$1"; }

step "rustfmt"
cargo fmt --all --check

step "clippy (native)"
cargo clippy --workspace --all-targets -- -D warnings

step "clippy (wasm32)"
cargo clippy -p ravenrs -p ravenrs-platform-web -p ravenrs-report -p ravenrs-report-pdf -p ravenrs-report-xlsx -p hello-world -p component-gallery -p report-designer --target wasm32-unknown-unknown -- -D warnings

step "tests: unit, behavior, property, doc, GPU pixel and golden-image (GPU tests skip without an adapter)"
cargo test --workspace

step "optional features are optional"
cargo check -p ravenrs --no-default-features

step "minimum supported Rust version (1.88)"
if rustup toolchain list | grep -q '^1.88'; then
    cargo +1.88 check --workspace --exclude hello-world --exclude component-gallery --exclude report-designer
else
    echo "skipped: install it with 'rustup toolchain install 1.88'"
fi

step "dependencies: advisories, licenses, sources (cargo-deny)"
if command -v cargo-deny >/dev/null; then
    cargo deny check
else
    echo "skipped: install it with 'cargo install cargo-deny --locked'"
fi

step "benchmarks compile"
cargo bench --workspace --no-run --quiet

step "API docs (broken links fail)"
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace --exclude hello-world --exclude component-gallery --exclude report-designer --quiet

step "web builds"
./scripts/build-web.sh hello-world
./scripts/build-web.sh component-gallery
./scripts/build-web.sh report-designer

printf '\n\033[1;32mAll checks passed.\033[0m\n'
