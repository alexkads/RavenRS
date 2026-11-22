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

A component that implements it also has the stories the design tool needs (#768):

- at least one story;
- for a setting of choices (`ArgType::choice`) of up to 12 options, a story for each option (not
  for `icon`, a sample of the icon set that the card of `Icon` lists whole): one
  that sets it, the default when some story leaves it as it is, or a story drawn its own way
  (`story_with`) whose name says the setting, as "All sizes";
- for an on/off setting of a state (`disabled`, `read_only`, `loading`, `error`), a story with it on.

What does not have them yet is listed in pendentes.json with `"conteudo": true` and its issue; such
an entry is refused once the component has them.

Nothing here needs a network, a GPU or a compiler.
"""
import json
import re
import sys

from component_inventory import root, widget_components

PENDING = root / "tools/raven-component-book/pendentes.json"
CATALOG = root / "site/components/index.html"
IMPL = re.compile(r"^impl(?:<[^>]*>)? ComponentBook for (\w+)", re.M)
LISTED = re.compile(r"\b(\w+)(?:::<[^>]*>)?::book\(\)|<crate::(\w+)(?:<[^>]*>)? as [\w:]*ComponentBook>::book\(\)")
TITLE = re.compile(r'Meta::new\(\s*"([^"/]+)/(\w+)"')
CHOICE = re.compile(r'ArgType::choice\(\s*"(\w+)"\s*,\s*(.+?)\s*,\s*"([^"]*)"\s*\)', re.S)
BOOL = re.compile(r'ArgType::bool\(\s*"(\w+)"\s*,\s*(true|false)')
CONST = re.compile(r"const (\w+): (?:\[&(?:'static )?str; \d+\]|&\[&(?:'static )?str\]) =\s*&?\[(.*?)\];", re.S)
STATES = ("disabled", "read_only", "loading", "error")
MAX_OPTIONS = 12


def calls(text, name):
    """The text inside the parentheses of each `.name(` call, parentheses balanced."""
    out, at = [], 0
    while (start := text.find(f".{name}(", at)) >= 0:
        i, depth = start + len(name) + 2, 1
        while i < len(text) and depth:
            depth += {"(": 1, ")": -1}.get(text[i], 0)
            i += 1
        out.append(text[start + len(name) + 2 : i - 1])
        at = i
    return out


def options_of(expr, consts):
    """The options a choice is given: a literal list, a constant of the file or a slice of one."""
    if literal := re.fullmatch(r'&\[(.*)\]', expr.strip(), re.S):
        return re.findall(r'"([^"]*)"', literal.group(1))
    if named := re.fullmatch(r'&?(?:\w+::)*(\w+)(?:\[(\d*)\.\.(\d*)\])?', expr.strip()):
        values = consts.get(named.group(1))
        if values is None:
            return None
        start = int(named.group(2) or 0)
        end = int(named.group(3)) if named.group(3) else len(values)
        return values[start:end] if named.group(2) is not None or named.group(3) else values
    return None


def content_problems(text, consts):
    """What a `ComponentBook` lacks for the design tool, from its source."""
    stories = [("story", c) for c in calls(text, "story")] + [("story_with", c) for c in calls(text, "story_with")]
    if not stories:
        return ["no story"]
    story_names = [(kind, (re.match(r'\s*"([^"]*)"', c) or [None, ""])[1], c) for kind, c in stories]
    problems = []
    for name, expr, default in CHOICE.findall(text):
        if name == "icon":
            continue
        options = options_of(expr, consts)
        if not options or len(options) > MAX_OPTIONS:
            continue
        words = name.replace("_", " ").lower()
        if any(kind == "story_with" and words in title.lower() for kind, title, _ in story_names):
            continue
        seen = set()
        for _, _, body in story_names:
            set_here = re.findall(rf'\.with\(\s*"{name}"\s*,\s*(?:ArgValue::choice\(\s*)?"([^"]*)"', body)
            seen.update(set_here)
            if not set_here:
                seen.add(default)
        missing = [o for o in options if o not in seen]
        if missing:
            problems.append(f"no story with {name} = {', '.join(missing)}")
    for name, default in BOOL.findall(text):
        if name not in STATES:
            continue
        words = name.replace("_", " ").lower()
        sets = [re.search(rf'\.with\(\s*"{name}"\s*,\s*(true|false)', body) for _, _, body in story_names]
        # On when a story turns it on, or leaves on a setting that starts on.
        on = any((m and m.group(1) == "true") or (not m and default == "true") for m in sets)
        titled = any(words in title.lower() for _, title, _ in story_names)
        if not (on or titled):
            problems.append(f"no story with {name} on")
    return problems


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


def content_verdict(name, crate, place, lacks, rcb):
    """What the guard says of a component that implements `ComponentBook`, given what its stories
    lack and its entry in pendentes.json: its problems, and the entry left to check as a component
    without `ComponentBook` (`None` once a content entry answered for it)."""
    problems = []
    if rcb and rcb.get("conteudo"):
        if not lacks:
            problems.append(
                f"{name} ({crate}) already has the stories the design tool needs: take it out of "
                f"tools/raven-component-book/pendentes.json"
            )
        elif not isinstance(rcb.get("issue"), int):
            problems.append(f"{name} ({crate}): a pending component needs the number of its issue")
        return problems, None
    if lacks:
        problems.append(
            f"{name} ({crate}, {place}): {'; '.join(lacks)} — give it the stories or list it with "
            f'"conteudo": true and its issue in tools/raven-component-book/pendentes.json'
        )
    return problems, rcb


def self_test():
    """The rules on made-up sources: a refusal, an acceptance and a pending entry refused."""
    source = '''
        impl ComponentBook for Chip {
            fn book() -> Meta {
                Meta::new("Forms/Chip", render)
                    .arg(ArgType::choice("tone", &["Neutral", "Accent", "Danger"], "Neutral"))
                    .arg(ArgType::bool("disabled", false))
                    .story("Neutral", Args::new())
            }
        }'''
    lacks = content_problems(source, {})
    assert lacks == ["no story with tone = Accent, Danger", "no story with disabled on"], lacks
    refused, _ = content_verdict("Chip", "kit", "book.rs:2", lacks, None)
    assert len(refused) == 1 and "Chip" in refused[0] and "Accent, Danger" in refused[0], refused
    pending = {"crate": "kit", "tipo": "Chip", "issue": 768, "conteudo": True}
    accepted, left = content_verdict("Chip", "kit", "book.rs:2", lacks, pending)
    assert accepted == [] and left is None, accepted
    complete = source.replace(
        '.story("Neutral", Args::new())',
        '.story("Neutral", Args::new()).story("Accent", Args::new().with("tone", ArgValue::choice("Accent")))'
        '.story("Danger", Args::new().with("tone", ArgValue::choice("Danger"))).story("Disabled", Args::new().with("disabled", true))',
    )
    assert content_problems(complete, {}) == []
    stale, _ = content_verdict("Chip", "kit", "book.rs:2", [], pending)
    assert len(stale) == 1 and "take it out" in stale[0], stale
    # A story drawn its own way, named after the setting, shows every option.
    assert content_problems(source.replace('.story("Neutral", Args::new())', '.story_with("All tones", Args::new(), all).story("Disabled", Args::new().with("disabled", true))'), {}) == []
    print("rcb-coverage: self-test ok")


def main():
    if "--self-test" in sys.argv:
        self_test()
        return
    summary = "--summary" in sys.argv
    components = widget_components()
    entries = json.loads(PENDING.read_text())["pendentes"]
    pending = {(e["crate"], e["tipo"]): e for e in entries}
    family_of = families()
    implemented, listed, titles, content = {}, set(), {}, {}
    # The lists of options of every book, as a choice may take another file's.
    shared = {}
    for _, _, text in book_sources():
        for k, v in CONST.findall(text):
            shared.setdefault(k, re.findall(r'"([^"]*)"', v))
    for crate, path, text in book_sources():
        consts = {**shared, **{k: re.findall(r'"([^"]*)"', v) for k, v in CONST.findall(text)}}
        impls = list(IMPL.finditer(text))
        for i, found in enumerate(impls):
            line = text[: found.start()].count("\n") + 1
            implemented[(crate, found.group(1))] = f"{path.relative_to(root)}:{line}"
            end = impls[i + 1].start() if i + 1 < len(impls) else len(text)
            content[(crate, found.group(1))] = content_problems(text[found.start() : end], consts)
        listed.update(a or b for a, b in LISTED.findall(text))
        for found in TITLE.finditer(text):
            titles[found.group(2)] = (found.group(1), f"{path.relative_to(root)}:{text[: found.start()].count(chr(10)) + 1}")

    problems = []
    for key, (path, line) in sorted(components.items()):
        crate, name = key
        place = f"{path.relative_to(root)}:{line}"
        rcb = pending.get(key)
        if key in implemented:
            found, rcb = content_verdict(name, crate, implemented[key], content.get(key, []), rcb)
            problems.extend(found)
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
