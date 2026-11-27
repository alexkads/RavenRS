#!/usr/bin/env python3
"""Refuses fixed words in what the components show or read aloud (#775).

    scripts/check-fixed-texts.py            # checks; exit 1 with file:line of each one
    scripts/check-fixed-texts.py --self-test

Every word a component of the `ravenrs-widgets-*` crates shows or reads aloud comes from `Labels`
(`cx.labels()`), so an app in Portuguese does not hear "options" and one in English does not read
"Duas saídas não se ligam". This looks, outside tests, stories (`book`), test drivers (`testing.rs`)
and the playground (`live*.rs`), for a literal with letters in `.name("…")`, `.description("…")`,
`.placeholder("…")`, `Err("…")`, `Button::new("…")` and `Text::new("…")`, and for a `format!`
that gives one of them and whose text has a loose word. An exception goes in ALLOWED with why.
"""
import pathlib
import re
import sys

root = pathlib.Path(__file__).resolve().parent.parent

CALLS = re.compile(
    r"""(\.name|\.description|\.placeholder|Err|Button::new|Text::new)\(\s*"([^"\\]*(?:\\.[^"\\]*)*)"\s*[,)]"""
)
FORMAT = re.compile(
    r"""(?:\.name|\.description|\.placeholder|Err|Button::new|Text::new|(?:name|description) = Some)\(\s*format!\(\s*"([^"\\]*(?:\\.[^"\\]*)*)\""""
)
# A word: three letters or more outside a `{…}` placeholder.
WORD = re.compile(r"[A-Za-zÀ-ÿ]{3,}")

# file (from the root) and the literal, with why each one stays.
ALLOWED = {
    # Ids and kinds, never shown: they name what a drag carries.
    ("crates/ravenrs-widgets-base/src/patch_panel.rs", "patch-panel-{id}"): "the kind of a drag, not shown",
}


def shown_words(text):
    """The words of `text` outside placeholders such as `{name}` or `{0:>2}`."""
    return WORD.findall(re.sub(r"\{[^}]*\}", "", text))


def skipped(path):
    parts = path.parts
    name = path.name
    return (
        "tests" in parts
        or "book" in parts
        or name == "book.rs"
        or name == "testing.rs"
        or name.startswith("live")
        or "benches" in parts
    )


def code_lines(text):
    """The lines of `text` before a `#[cfg(test)]` module, without `//` comments."""
    for number, line in enumerate(text.splitlines(), 1):
        if line.strip().startswith("#[cfg(test)]"):
            return
        yield number, line.split("//")[0] if '"' not in line.split("//")[0][-1:] else line


def problems_in(path, text):
    found = []
    rel = path.relative_to(root).as_posix() if path.is_absolute() else path.as_posix()
    for number, line in code_lines(text):
        for _, literal in CALLS.findall(line):
            if shown_words(literal) and (rel, literal) not in ALLOWED:
                found.append(f"{rel}:{number}: \"{literal}\" — use cx.labels()")
        for literal in FORMAT.findall(line):
            if shown_words(literal) and (rel, literal) not in ALLOWED:
                found.append(f"{rel}:{number}: format!(\"{literal}\") — use cx.labels()")
    return found


def self_test():
    sample = pathlib.Path("crates/ravenrs-widgets-base/src/x.rs")
    bad = 'cx.semantics(n, Semantics::new(Role::Group).name("Pagination"));\nlet a = x.name(format!("{} options", t));'
    good = 'cx.semantics(n, Semantics::new(Role::Group).name(labels.pagination.clone()));\nlet a = x.name(format!("{a} {b}"));\ncx.test_id(n, &format!("day:{d}"));'
    assert len(problems_in(sample, bad)) == 2, problems_in(sample, bad)
    assert problems_in(sample, good) == [], problems_in(sample, good)
    assert problems_in(sample, '#[cfg(test)]\nmod t { fn f() { Text::new("Hello"); } }') == []
    print("check-fixed-texts self-test: 3 cases")


def main():
    if "--self-test" in sys.argv:
        return self_test()
    found = []
    for crate in sorted((root / "crates").glob("ravenrs-widgets-*")):
        for path in sorted((crate / "src").rglob("*.rs")):
            if not skipped(path.relative_to(crate)):
                found += problems_in(path, path.read_text())
    if found:
        print("Fixed words in what the components show or say (#775):")
        print("\n".join(f"  {line}" for line in found))
        sys.exit(1)
    print("no fixed words in the components")


main()
