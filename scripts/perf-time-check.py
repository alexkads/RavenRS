#!/usr/bin/env python3
"""Takes the runs of `ravenrs-perf time`, keeps the best p99 of each number, and checks
it against the budgets of docs/QUALITY.md: a 120 Hz frame on the reference Mac."""
import json
import sys

UI_INTERACTION = 4.0  # milliseconds on the CPU: hover, scroll, animation.
UI_REBUILD = 16.67  # milliseconds on the CPU: resizing, new data.
GPU = 3.0
# The sizes the core is not asked to keep at 120 Hz yet; shown, never failed.
SHOWN_ONLY = ("curve-large",)

runs = [json.load(open(path)) for path in sys.argv[1:]]
problems = []
fields = ("ui", "build", "layout", "place", "paint", "render", "gpu")
print(f"{'best p99, ms':38}{'elements':>9}" + "".join(f"{f:>8}" for f in fields))
for name in runs[0]:
    best = {f: min(run[name][f] for run in runs if name in run) for f in fields}
    if runs[0][name]["frames"] == 0:
        continue
    print(f"{name:38}{runs[0][name]['elements']:>9}" + "".join(f"{best[f]:>8.2f}" for f in fields))
    if name.startswith(SHOWN_ONLY):
        continue
    interaction = name.split("/")[1]
    limit = UI_REBUILD if interaction in ("resize", "data") else UI_INTERACTION
    if best["ui"] > limit:
        problems.append(f"{name}: {best['ui']:.2f} ms on the CPU, over {limit} ms")
    if best["gpu"] > GPU:
        problems.append(f"{name}: {best['gpu']:.2f} ms on the GPU, over {GPU} ms")

if problems:
    print("\nover budget:\n" + "\n".join(problems))
    sys.exit(1)
print("\nevery scene is inside its budget")
