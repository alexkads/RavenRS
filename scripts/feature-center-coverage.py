#!/usr/bin/env python3
"""Checks that the feature center shows every component with every feature (#580).

    scripts/feature-center-coverage.py            # checks; exit 1 with what is missing
    scripts/feature-center-coverage.py --summary  # also lists the components still waiting

tools/raven-feature-center/recursos.json has one entry per component: each public view of the
widgets' crates and each type of ravenrs-app* set up by methods, as scripts/component_inventory.py
finds them. An entry is either

- shown: {"crate", "tipo", "pagina", "recursos"}. "recursos" are the type's `pub fn`, exactly; each
  one is called in tools/raven-feature-center/src (`.name(` or `::name(`), and the page, as the
  navigation calls it, is visited in tests/suite/tour.rs; or
- waiting: {"crate", "tipo", "pendente": true, "issue", "conhecidos"}. "conhecidos" are the `pub fn`
  it had when it started waiting. A new one means the component changed, and a component that
  changes leaves the waiting list in the same pull request, with every feature shown.

A public component with no entry fails: a new component comes with its page. The skill
ravenrs-feature-center says how to build the page.
"""
import json
import re
import sys

from component_inventory import app_builders, built_components, public_methods_at, root, widget_components

CENTER = root / "tools/raven-feature-center"
MANIFEST = CENTER / "recursos.json"
TOUR = CENTER / "tests/suite/tour.rs"


def place(path, line):
    return f"{path.relative_to(root)}:{line}"


def main():
    entries = json.loads(MANIFEST.read_text())["componentes"]
    code = {**widget_components(), **built_components(), **app_builders()}
    by_key = {}
    problems = []
    for entry in entries:
        key = (entry["crate"], entry["tipo"])
        if key in by_key:
            problems.append(f"{entry['tipo']} ({entry['crate']}): two entries in recursos.json")
        by_key[key] = entry

    center_source = "\n".join(path.read_text() for path in sorted((CENTER / "src").rglob("*.rs")))
    tour = TOUR.read_text()
    waiting = []

    for key, (path, line) in sorted(code.items()):
        crate, name = key
        entry = by_key.get(key)
        if entry is None:
            problems.append(
                f"{place(path, line)}: {name} ({crate}) is a public component with no entry in "
                "tools/raven-feature-center/recursos.json: give it a page in the feature center that shows "
                "every feature (skill ravenrs-feature-center)"
            )
            continue
        methods = public_methods_at(crate, name)
        if entry.get("pendente"):
            waiting.append(entry)
            known = set(entry.get("conhecidos", []))
            new = sorted(set(methods) - known)
            gone = sorted(known - set(methods))
            for method in new:
                where, at, _ = methods[method]
                problems.append(
                    f"{place(where, at)}: {name}::{method} is new, and {name} still waits (#{entry.get('issue')}): "
                    f"a component that changes is shown whole, so take {name} off the waiting list in "
                    "recursos.json and show every one of its features in the feature center"
                )
            if gone:
                problems.append(
                    f"{name} ({crate}) no longer has {', '.join(gone)}: take them out of its \"conhecidos\""
                )
            continue
        declared = set(entry.get("recursos", []))
        for method in sorted(set(methods) - declared):
            where, at, _ = methods[method]
            problems.append(
                f"{place(where, at)}: {name}::{method} is not shown: call it in the page "
                f"\"{entry.get('pagina')}\" and add it to the \"recursos\" of {name}"
            )
        gone = sorted(declared - set(methods))
        if gone:
            problems.append(f"{name} ({crate}) no longer has {', '.join(gone)}: take them out of its \"recursos\"")
        for method in sorted(declared & set(methods)):
            if not re.search(rf"(\.|::){method}\s*(::<[^>]*>)?\(", center_source):
                problems.append(f"{name}::{method} is declared but never called in tools/raven-feature-center/src")
        page = entry.get("pagina")
        if not page:
            problems.append(f"{name} ({crate}) is shown but has no \"pagina\"")
        elif f'"{page}"' not in tour:
            problems.append(f"{name}: the page \"{page}\" has no step in tools/raven-feature-center/tests/suite/tour.rs")

    for key, entry in sorted(by_key.items()):
        if key not in code:
            problems.append(f"{entry['tipo']} ({entry['crate']}) is no longer a public component: take it out of recursos.json")

    if problems:
        print("\n".join(problems), file=sys.stderr)
        print(f"\n{len(problems)} problem(s). The skill ravenrs-feature-center says what to do.", file=sys.stderr)
        sys.exit(1)
    shown = len(code) - len(waiting)
    print(f"{len(code)} components: {shown} shown with every feature, {len(waiting)} waiting")
    if "--summary" in sys.argv:
        for entry in waiting:
            print(f"  waiting: {entry['tipo']} ({entry['crate']}), #{entry.get('issue')}")


if __name__ == "__main__":
    main()
