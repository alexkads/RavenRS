#!/usr/bin/env python3
"""The widgets' crates keep their layers: the base uses no other, the kit only the base, and each
family only the base and the kit. Two families that need the same thing take it from below, so
editing one never rebuilds another. Only `[dependencies]` count: a dev-dependency on the facade,
for the examples in the documentation, does not bring code in.

    scripts/check-widget-layers.py       fails, naming the dependency, when a layer is skipped
"""

import pathlib
import re
import sys

root = pathlib.Path(__file__).resolve().parent.parent
ALLOWED = {
    "ravenrs-widgets-base": set(),
    "ravenrs-widgets-kit": {"ravenrs-widgets-base"},
}
FAMILY = {"ravenrs-widgets-base", "ravenrs-widgets-kit"}

problems = []
crates = sorted(p.parent.name for p in root.glob("crates/ravenrs-widgets-*/Cargo.toml"))
for crate in crates:
    manifest = (root / "crates" / crate / "Cargo.toml").read_text()
    section = manifest.split("\n[dependencies]\n", 1)[-1].split("\n[", 1)[0]
    used = set(re.findall(r"^(ravenrs-widgets(?:-[\w-]+)?)\b", section, re.M))
    allowed = ALLOWED.get(crate, FAMILY)
    for dependency in sorted(used - allowed):
        layer = "the base" if crate == "ravenrs-widgets-base" else "the kit" if crate == "ravenrs-widgets-kit" else "a family"
        problems.append(
            f"crates/{crate}/Cargo.toml: {layer} depends on {dependency}; what both need goes down to "
            "ravenrs-widgets-base or ravenrs-widgets-kit"
        )

if problems:
    sys.exit("\n".join(problems))
print(f"{len(crates)} widget crates keep their layers")
