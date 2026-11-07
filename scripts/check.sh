#!/usr/bin/env bash
# The project's checks, run locally: it uses no hosted CI.
#
#   scripts/check.sh          quick, for every change and pull request: formatting, then lints and
#                             tests of the packages the change reaches (scripts/changed-packages.py)
#   scripts/check.sh --full   everything, before a release: the whole workspace, wasm32, optional
#                             features, the minimum Rust version, dependencies, benchmarks, API docs
#                             and the web builds
#
# The performance gate runs only when asked for: RAVENRS_GATE=1 scripts/check.sh --full
set -euo pipefail
cd "$(dirname "$0")/.."

step() { printf '\n\033[1m==> %s\033[0m\n' "$1"; }
started=$SECONDS
done_in() { printf '\n\033[1;32m%s in %dm%02ds.\033[0m\n' "$1" $(((SECONDS - started) / 60)) $(((SECONDS - started) % 60)); }

step "rustfmt"
cargo fmt --all --check

if [ "${1:-}" != "--full" ]; then
    packages=$(./scripts/changed-packages.py)
    if [ -z "$packages" ]; then
        echo "no package changed against ${CHECK_BASE:-origin/master}: nothing to build"
        done_in "Quick checks passed"
        exit 0
    fi
    if [ "$packages" = "ALL" ]; then
        selection=(--workspace)
        echo "something every package builds with changed: the whole workspace"
    else
        selection=()
        for package in $packages; do selection+=(-p "$package"); done
        echo "packages reached: $(echo "$packages" | tr '\n' ' ')"
    fi
    step "clippy (native), on what the change reaches"
    cargo clippy "${selection[@]}" --all-targets -- -D warnings
    step "tests, on what the change reaches"
    cargo test "${selection[@]}"
    done_in "Quick checks passed (before a release: scripts/check.sh --full)"
    exit 0
fi

step "clippy (native)"
cargo clippy --workspace --all-targets -- -D warnings

step "clippy (wasm32)"
cargo clippy -p ravenrs -p ravenrs-platform-web -p ravenrs-report -p ravenrs-report-pdf -p ravenrs-report-xlsx -p ravenrs-report-viewer -p ravenrs-grid-export -p ravenrs-app -p ravenrs-app-ui -p ravenrs-app-library -p ravenrs-app-report -p ravenrs-report-designer -p quick-app -p quick-plan -p feature-center -p calculator -p hello-world -p component-gallery -p report-designer --target wasm32-unknown-unknown -- -D warnings

step "tests: unit, behavior, property, doc, GPU pixel and golden-image (GPU tests skip without an adapter)"
cargo test --workspace

step "optional features are optional"
cargo check -p ravenrs --no-default-features
cargo check -p ravenrs-widgets --no-default-features
if cargo tree -p ravenrs --no-default-features -e normal | grep -E 'ravenrs-graphics|tiny-skia'; then
    echo "an app without the charts feature must not build the rasterizer" >&2
    exit 1
fi

step "minimum supported Rust version (1.88)"
if rustup toolchain list | grep -q '^1.88'; then
    cargo +1.88 check --workspace --exclude ravenrs-app-sqlite --exclude ravenrs-movie-video --exclude channel-video --exclude calculator --exclude hello-world --exclude quick-app --exclude quick-plan --exclude feature-center --exclude component-gallery --exclude report-designer --exclude grid-poc --exclude family-finance --exclude ravenrs-perf
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

# Only when asked: RAVENRS_GATE=1 ./scripts/check.sh --full
if [ "${RAVENRS_GATE:-0}" = "1" ]; then
    step "performance gate: exact counters against perf/baseline.json"
    cargo run --release -q -p ravenrs-perf -- gate
fi

step "API docs (broken links fail)"
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace --exclude calculator --exclude hello-world --exclude quick-app --exclude quick-plan --exclude feature-center --exclude component-gallery --exclude report-designer --exclude grid-poc --exclude family-finance --exclude channel-video --exclude ravenrs-perf --quiet
./scripts/check-component-list.py

step "web builds"
./scripts/build-web.sh hello-world
./scripts/build-web.sh calculator
./scripts/build-web.sh quick-app
./scripts/build-web.sh quick-plan
./scripts/build-web.sh feature-center
./scripts/build-web.sh component-gallery
./scripts/build-web.sh report-designer

done_in "All checks passed"
