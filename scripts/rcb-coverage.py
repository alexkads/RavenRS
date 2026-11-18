#!/usr/bin/env python3
"""Checks that every component follows RCB, the Raven Component Book (#754).

    scripts/rcb-coverage.py            # checks; exit 1 with what is missing
    scripts/rcb-coverage.py --summary  # also lists the components still pending, by issue

A component follows RCB when it implements `ravenrs_widgets::book::ComponentBook`: its place in
the book, its settings and its stories, as a Storybook `*.stories` file. RavenComponentBook
(tools/raven-component-book) draws, measures and exports them for the design tool, so a component
without it reaches the design tool with one story and no settings, and the design invents.

Each public view of the widgets' crates (scripts/component_inventory.py) either

- implements `ComponentBook`, in the `book` module of its crate, is listed by that module's
  `stories()` (else the book never shows it), and its `Meta` title starts with its family as
  site/components/index.html names it; or
- is pending: tools/raven-component-book/pendentes.json lists it with the issue that gives it its
  stories, `{"crate", "tipo", "issue"}`. A component that implements the trait leaves that list in
  the same pull request. The list is the book's own, apart from the feature center's recursos.json,
  so the two can change in parallel without stepping on each other.

Nothing here needs a network, a GPU or a compiler.
"""
import json
import re
import sys

from component_inventory import root, widget_components

PENDING = root / "tools/raven-component-book/pendentes.json"
CATALOG = root / "site/components/index.html"
IMPL = re.compile(r"^impl(?:<[^>]*>)? ComponentBook for (\w+)", re.M)
LISTED = re.compile(r"\b(\w+)(?:::<[^>]*>)?::book\(\)")
TITLE = re.compile(r'Meta::new\(\s*"([^"/]+)/(\w+)"')


def families():
    """The family of each component, as the site's list names it."""
    out, family = {}, None
    for line in CATALOG.read_text().splitlines():
        if found := re.search(r'<h2 id="[^"]+">.*?#</a>(.*?)</h2>', line):
            family = found.group(1)
        elif (found := re.search(r'data-api="[^"]*">(\w+)</a>', line)) and family:
            out.setdefault(found.group(1), family)
    return out


def book_sources():
    """(crate, path, text) of every file of a crate's `book` module."""
    for crate in sorted((root / "crates").iterdir()):
        src = crate / "src"
        for path in [src / "book.rs", *sorted((src / "book").rglob("*.rs"))]:
            if path.exists():
                yield crate.name, path, path.read_text()


def main():
    summary = "--summary" in sys.argv
    components = widget_components()
    entries = json.loads(PENDING.read_text())["pendentes"]
    pending = {(e["crate"], e["tipo"]): e for e in entries}
    family_of = families()
    implemented, listed, titles = {}, set(), {}
    for crate, path, text in book_sources():
        for found in IMPL.finditer(text):
            line = text[: found.start()].count("\n") + 1
            implemented[(crate, found.group(1))] = f"{path.relative_to(root)}:{line}"
        listed.update(LISTED.findall(text))
        for found in TITLE.finditer(text):
            titles[found.group(2)] = (found.group(1), f"{path.relative_to(root)}:{text[: found.start()].count(chr(10)) + 1}")

    problems = []
    for key, (path, line) in sorted(components.items()):
        crate, name = key
        place = f"{path.relative_to(root)}:{line}"
        rcb = pending.get(key)
        if key in implemented:
            if rcb:
                problems.append(
                    f"{name} ({crate}) implements ComponentBook ({implemented[key]}) and is still in "
                    f"tools/raven-component-book/pendentes.json: take it out in this pull request"
                )
            if name not in listed:
                problems.append(
                    f"{name} ({crate}) implements ComponentBook but the stories() of its crate's book does not "
                    f"list {name}::book(): the book would never show it"
                )
            family = family_of.get(name)
            title = titles.get(name)
            if family and title and title[0] != family:
                problems.append(
                    f"{name}: its Meta title says \"{title[0]}/{name}\" ({title[1]}), the site lists it under "
                    f"\"{family}\": the book and the design system group it as the site does"
                )
        elif not rcb:
            problems.append(
                f"{name} ({crate}, {place}) has no ComponentBook: give it its stories "
                f"(tools/raven-component-book/README.md) or list it, with its issue, in "
                f"tools/raven-component-book/pendentes.json"
            )
        elif not isinstance(rcb.get("issue"), int):
            problems.append(f"{name} ({crate}): a pending component needs the number of the issue that gives it its stories")
    for key in sorted(set(pending) - set(components)):
        problems.append(f"{key[1]} ({key[0]}) is in tools/raven-component-book/pendentes.json but is not a public component")

    done = sum(1 for key in components if key in implemented)
    if problems:
        print("components that do not follow RCB, the Raven Component Book (#754):")
        for p in problems:
            print(f"  {p}")
        sys.exit(1)
    print(f"{len(components)} components: {done} follow RCB, {len(components) - done} pending")
    if summary:
        by_issue = {}
        for key, rcb in pending.items():
            by_issue.setdefault(rcb["issue"], []).append(key[1])
        for issue, names in sorted(by_issue.items()):
            print(f"  #{issue}: {len(names)} — {', '.join(sorted(names))}")


main()
