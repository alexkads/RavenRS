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
cargo clippy -p raven -p raven-platform-web -p hello-world --target wasm32-unknown-unknown -- -D warnings

step "tests (GPU tests skip themselves without an adapter)"
cargo test --workspace

step "web build"
./scripts/build-web.sh hello-world

printf '\n\033[1;32mAll checks passed.\033[0m\n'
