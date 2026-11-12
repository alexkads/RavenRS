#!/usr/bin/env python3
"""Every file in a tests/suite/ folder is declared in its main.rs.

The integration tests of a package live in one binary, tests/suite/main.rs, so they link once. A
file placed next to it without `mod name;` in main.rs is never compiled, and its tests silently stop
running; this check fails instead.
"""
import pathlib
import re
import sys

root = pathlib.Path(__file__).resolve().parent.parent
missing = []
for main in sorted(root.glob("*/*/tests/suite/main.rs")):
    declared = set(re.findall(r"^\s*mod\s+(\w+)\s*;", main.read_text(), re.M))
    suite = main.parent
    present = {p.stem for p in suite.glob("*.rs") if p.name != "main.rs"}
    present |= {p.name for p in suite.iterdir() if p.is_dir() and (p / "mod.rs").exists()}
    for name in sorted(present - declared):
        missing.append(f"{suite.relative_to(root)}/{name}: add `mod {name};` to main.rs")
loose = sorted(
    p.relative_to(root)
    for p in root.glob("*/*/tests/*.rs")
    if (p.parent / "suite" / "main.rs").exists()
)
for p in loose:
    missing.append(f"{p}: move it into tests/suite/ and declare it in main.rs (one binary per package)")
if missing:
    print("test files that would not run with the rest:", *missing, sep="\n  ")
    sys.exit(1)
