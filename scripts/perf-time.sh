#!/usr/bin/env bash
# How long frames take, against the budgets in docs/QUALITY.md. Run before each release,
# plugged in, on the reference Mac. It is not part of scripts/check.sh: time on a laptop
# is noisy, and small regressions are what the gate's exact counters catch.
#
# Noise almost only adds time, so the estimate is the best of three separate processes.
set -euo pipefail
cd "$(dirname "$0")/.."

if command -v pmset >/dev/null && ! pmset -g batt | grep -q "AC Power"; then
    echo "invalid: on battery the numbers say nothing. Plug in and run again." >&2
    exit 2
fi

cargo build --release -q -p ravenrs-perf
binary="${CARGO_TARGET_DIR:-target}/release/ravenrs-perf"
folder="$(mktemp -d)"
trap 'rm -rf "$folder"' EXIT
for run in 1 2 3; do
    "$binary" time "$@" > "$folder/$run.json"
done
./scripts/perf-time-check.py "$folder"/1.json "$folder"/2.json "$folder"/3.json
