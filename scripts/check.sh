#!/usr/bin/env bash
# The project's checks, run locally: it uses no hosted CI.
#
#   scripts/check.sh          quick, for every change and pull request (a few minutes): formatting;
#                             clippy on every package the change reaches, at any depth (that also
#                             proves they compile); tests of the packages changed and of those that
#                             depend on them directly. scripts/changed-packages.py says which: it
#                             reads Cargo.toml and Cargo.lock entry by entry, so a new crate or a
#                             version bump is not the whole workspace.
#   scripts/check.sh --full   everything, before a release: the whole workspace and all its tests,
#                             wasm32, optional features, the minimum Rust version, dependencies, API
#                             docs and the web builds
#
# Benchmarks and the performance gate run only when asked for: RAVENRS_GATE=1 scripts/check.sh --full
#
# Tests run on cargo-nextest, which every contributor installs once: cargo install cargo-nextest
# --locked. It runs every test binary at once, where cargo test runs them one after the other, each
# test in a process of its own. Doctests, which it does not run, go through cargo test --doc.
#
# One build directory per worktree. Cargo does not put a local crate's path in its artifacts' hash,
# so two worktrees sharing a CARGO_TARGET_DIR take each other's builds: a doctest that "can't find
# crate", a derive compiled from the other branch. Give each worktree its own CARGO_TARGET_DIR (in
# its session's scratchpad) and delete it when the work is done.
set -euo pipefail
cd "$(dirname "$0")/.."

step() { printf '\n\033[1m==> %s\033[0m\n' "$1"; }
started=$SECONDS
done_in() { printf '\n\033[1;32m%s in %dm%02ds.\033[0m\n' "$1" $(((SECONDS - started) / 60)) $(((SECONDS - started) % 60)); }

# Warn when the build directory last served another checkout of this repository.
target=$(cargo metadata --format-version 1 --no-deps --offline | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')
mkdir -p "$target"
if [ -f "$target/.ravenrs-checkout" ] && [ "$(cat "$target/.ravenrs-checkout")" != "$PWD" ]; then
    printf '\033[1;33mwarning: %s was last used by %s.\033[0m\n' "$target" "$(cat "$target/.ravenrs-checkout")" >&2
    echo "Worktrees that share a build directory reuse each other's local crates; give this one its own CARGO_TARGET_DIR." >&2
fi
echo "$PWD" >"$target/.ravenrs-checkout"

if ! command -v cargo-nextest >/dev/null; then
    echo "error: the checks run the tests on cargo-nextest. Install it once with:" >&2
    echo "    cargo install cargo-nextest --locked" >&2
    exit 1
fi

# Tests of the selected packages on nextest; doctests, which it does not run, through cargo test.
run_tests() {
    local -a selection=("$@")
    cargo nextest run "${selection[@]}" --no-tests=pass
    if [ "${selection[0]}" = "--workspace" ] || [ -n "${DOCUMENTED:-}" ]; then
        local -a documented=("${selection[@]}")
        if [ "${selection[0]}" != "--workspace" ]; then
            documented=()
            for package in $DOCUMENTED; do documented+=(-p "$package"); done
        fi
        cargo test --doc "${documented[@]}"
    fi
}

public_api() {
    step "the public API of ravenrs-widgets is unchanged (rustdoc JSON, on nightly)"
    ./scripts/public-api.py --check
}

lint_examples() {
    # The examples are the showcase, and the house follows its own rules.
    step "ravenrs-lint on the examples"
    for example in family-finance component-gallery; do
        cargo run -q -p ravenrs-lint -- -p "$example"
    done
}

step "rustfmt"
cargo fmt --all --check

step "modules do not use each other in loops longer than each crate allows (ravenrs-lint module_cycle)"
cargo run -q -p ravenrs-lint -- --workspace --only module_cycle

step "the widgets' crates keep their layers: no family depends on another"
./scripts/check-widget-layers.py

step "every test file runs: tests/suite/ modules are declared"
./scripts/check-test-suites.py

if [ "${1:-}" != "--full" ]; then
    step "what the change reaches"
    # What the selection leaves out goes unchecked, so the selection has tests of its own.
    python3 scripts/changed_packages_test.py >/dev/null 2>&1 || python3 scripts/changed_packages_test.py
    eval "$(./scripts/changed-packages.py --shell)"
    if [ "$ALL" = 1 ]; then
        echo "something every package builds with changed: the whole workspace"
        lint_examples
        public_api
        step "clippy (native), the whole workspace"
        cargo clippy --workspace --all-targets -- -D warnings
        step "tests, the whole workspace"
        run_tests --workspace
        done_in "Quick checks passed (before a release: scripts/check.sh --full)"
        exit 0
    fi
    if [ "$LINT" = 1 ]; then
        lint_examples
    fi
    # rustdoc on nightly takes a while: only when the change reaches the facade it reads.
    case " $REACHED " in *" ravenrs-widgets "*) public_api ;; esac
    if [ -z "$REACHED" ]; then
        echo "no package changed against ${CHECK_BASE:-origin/master}: nothing to build"
        done_in "Quick checks passed"
        exit 0
    fi
    echo "changed:         $TOUCHED"
    echo "tested:          $TESTED"
    echo "linted, at most: $REACHED"
    reached=()
    for package in $REACHED; do reached+=(-p "$package"); done
    tested=()
    for package in $TESTED; do tested+=(-p "$package"); done
    step "clippy (native), on every package the change reaches"
    cargo clippy "${reached[@]}" --all-targets -- -D warnings
    step "tests, on the packages changed and their direct dependents"
    run_tests "${tested[@]}"
    done_in "Quick checks passed (before a release: scripts/check.sh --full)"
    exit 0
fi

lint_examples
public_api

step "clippy (native)"
cargo clippy --workspace --all-targets -- -D warnings

step "CLI templates: every model and target compiles"
cargo test -p cargo-ravenrs --test cli -- --ignored

step "clippy (wasm32)"
cargo clippy -p ravenrs -p ravenrs-widgets -p ravenrs-widgets-base -p ravenrs-widgets-kit -p ravenrs-widgets-grid -p ravenrs-widgets-lists -p ravenrs-widgets-scheduler -p ravenrs-widgets-dock -p ravenrs-widgets-imaging -p ravenrs-widgets-charts -p ravenrs-widgets-chat -p ravenrs-platform-web -p ravenrs-report -p ravenrs-report-pdf -p ravenrs-report-xlsx -p ravenrs-report-viewer -p ravenrs-grid-export -p ravenrs-app -p ravenrs-app-i18n -p ravenrs-app-a11y -p ravenrs-app-ui -p ravenrs-app-remote -p ravenrs-app-library -p ravenrs-app-report -p ravenrs-report-designer -p quick-app -p quick-plan -p feature-center -p calculator -p hello-world -p component-gallery -p report-designer -p ravenrs-sheet -p ravenrs-spreadsheet -p spreadsheet -p tutorial-tasks -p tasks-sqlite -p ravenrs-barcode -p ravenrs-pos -p ravenrs-report-label --target wasm32-unknown-unknown -- -D warnings

# On their own: together with the rest, Cargo unifies features and rand_core asks for getrandom.
cargo clippy -p ravenrs-dfe -p ravenrs-nfe -p ravenrs-nfse -p ravenrs-pagamento --target wasm32-unknown-unknown -- -D warnings

step "tests: unit, behavior, property, doc, GPU pixel and golden-image (GPU tests skip without an adapter)"
run_tests --workspace

step "ravenrs-dfe and ravenrs-nfe with the real network code, against local servers only"
cargo test -p ravenrs-dfe -p ravenrs-nfe --all-features

step "optional features are optional"
cargo check -p ravenrs --no-default-features
cargo check -p ravenrs-widgets-base -p ravenrs-widgets-kit -p ravenrs-widgets-grid -p ravenrs-widgets-lists -p ravenrs-widgets-scheduler -p ravenrs-widgets-dock -p ravenrs-widgets-imaging -p ravenrs-widgets-charts -p ravenrs-widgets-chat -p ravenrs-widgets --no-default-features
if cargo tree -p ravenrs --no-default-features -e normal | grep -E 'ravenrs-graphics|tiny-skia'; then
    echo "an app without the charts feature must not build the rasterizer" >&2
    exit 1
fi

step "minimum supported Rust version (1.88)"
if rustup toolchain list | grep -q '^1.88'; then
    cargo +1.88 check --workspace --exclude ravenrs-movie-video --exclude channel-video --exclude tasks-sqlite --exclude ravenrs-sheet --exclude ravenrs-spreadsheet --exclude spreadsheet --exclude ravenrs-app-sqlite --exclude mini-pdv --exclude calculator --exclude hello-world --exclude quick-app --exclude quick-plan --exclude feature-center --exclude component-gallery --exclude report-designer --exclude grid-poc --exclude family-finance --exclude ravenrs-perf
else
    echo "skipped: install it with 'rustup toolchain install 1.88'"
fi

step "dependencies: advisories, licenses, sources (cargo-deny)"
if command -v cargo-deny >/dev/null; then
    cargo deny check
else
    echo "skipped: install it with 'cargo install cargo-deny --locked'"
fi

step "every example downloads from the playground as a project that builds"
cargo test -p component-gallery --test suite projects:: -- --ignored

# Only when asked: RAVENRS_GATE=1 ./scripts/check.sh --full
if [ "${RAVENRS_GATE:-0}" = "1" ]; then
    step "benchmarks compile"
    cargo bench --workspace --no-run --quiet

    step "performance gate: exact counters against perf/baseline.json"
    cargo run --release -q -p ravenrs-perf -- gate
fi

step "API docs (broken links fail)"
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace --features ravenrs-widgets/code --exclude channel-video --exclude mini-pdv --exclude calculator --exclude hello-world --exclude quick-app --exclude quick-plan --exclude feature-center --exclude component-gallery --exclude report-designer --exclude grid-poc --exclude family-finance --exclude ravenrs-perf --quiet
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
