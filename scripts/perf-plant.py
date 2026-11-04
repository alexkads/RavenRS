#!/usr/bin/env python3
"""Proves the performance gate works: plants one regression at a time in the runtime,
expects `ravenrs-perf gate` to fail on the counter it should move, and puts the code
back. Run by hand after changing the gate. Needs a clean crates/ravenrs-widgets/src/ui.rs."""
import pathlib
import subprocess
import sys

root = pathlib.Path(__file__).resolve().parent.parent
ui = root / "crates/ravenrs-widgets/src/ui.rs"
original = ui.read_text()

PAINT = "    fn paint_scene(&mut self) {\n"
PLANTS = {
    "an extra rebuild": (
        "builds",
        "            self.rebuild();\n            // The tree changed under a stationary pointer",
        "            self.rebuild();\n            self.rebuild();\n            // The tree changed under a stationary pointer",
    ),
    "an extra allocation": (
        "ui_allocations",
        PAINT,
        PAINT + "        std::hint::black_box(Vec::<u8>::with_capacity(64));\n",
    ),
    "extra shaping": (
        "shapes",
        PAINT,
        PAINT
        + '        std::hint::black_box(self.text.layout(&format!("planted {}", self.stats.paints), &Default::default()));\n',
    ),
}

failures = []
try:
    for name, (counter, old, new) in PLANTS.items():
        if old not in original:
            sys.exit(f"cannot plant {name}: the code it looks for is gone")
        ui.write_text(original.replace(old, new, 1))
        run = subprocess.run(
            ["cargo", "run", "--release", "-q", "-p", "ravenrs-perf", "--", "gate"],
            cwd=root,
            capture_output=True,
            text=True,
        )
        caught = run.returncode != 0 and f": {counter} was " in run.stderr
        print(f"{name}: {'caught' if caught else 'MISSED'}")
        if not caught:
            failures.append(name)
            print(run.stderr[-2000:])
finally:
    ui.write_text(original)

sys.exit(1 if failures else 0)
