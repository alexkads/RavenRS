#!/usr/bin/env python3
"""Turns one storytold project's crates, at one revision, into the module layout of its RavenRS crate.

    scripts/storytold/convert.py <project> <checkout> <revision> <out dir>

<project> is a key of projects.json (gridcraft, wordcraft, pdfcraft); <checkout> a clone of its
repository (a partial clone, `--filter=blob:none`, is enough). Run against the revision in the
crate's NOTICE, it reproduces the crate but for our own changes; that is how the conversion is
checked before a newer revision is merged in with merge.py. See the ravenrs-<project> skills.

For every source in projects.json (a crate of theirs, or a crate they vendor):

- <path>/src/** becomes src/<module>/**, and lib.rs becomes mod.rs;
- <path>/tests/it/** becomes tests/<module>_it/**, and <path>/tests/<x> becomes tests/<module>_<x>;
- `crate::` inside the source becomes `crate::<module>::`, and the source's crate name (`ident`)
  becomes `crate::<module>` in src/ and `<lib>::<module>` in tests/; with "precise", only where
  the name is a path or in a `use`, for names that are also words in prose (hayro, lopdf);
- a module may be nested (`third_party::hayro` lands in src/third_party/hayro/); "tests": false
  leaves a source's integration tests out;
- `feature = "x"` follows the source's "features": true is always on, false always off, a string
  renames it to one of our features; features not listed are left as they are;
- include_bytes!/include_str! of a relative path is pointed at where that file lands: inside a
  converted source, or, for files elsewhere in their repository (fonts, fixtures), under
  assets/<their path>, copied too (a path missing from their repository is left as it was);
- so does `concat!(env!("CARGO_MANIFEST_DIR"), "/<path>")`, read at run time;
- a root's `#![no_std]` and `#![doc = include_str!("../README.md")]` go: in a module they would
  warn, and their README is not our documentation;
- a macro the source exports (`#[macro_export]`) lands at our crate's root, so the source's
  root re-exports it, to be found where it was;
- the conditions left with `all()` or `any()` in them are folded: `#[cfg(<always>)]` goes,
  `cfg_attr(<always>, a)` becomes `#[a]`, `cfg!` becomes `true` or `false`;
- "unit_tests": false compiles out a source's `#[cfg(test)]` code, for vendored crates whose
  tests the project never ran;
- code blocks in doc comments are built from outside the crate: crate names in them become
  `<lib>::<module>`, and `crate::` is left as it is;
- "replace" lists regular expressions applied last, for the few lines no rule above covers.

Files outside src/ and tests/ (manifests, benches, examples) are not converted: the manifest is
ours. Then every .rs file is formatted with the crate's rustfmt.toml, since the paths changed
length and lines break elsewhere.
"""
import json
import os
import posixpath
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
CRATE_ONLY = re.compile(r'^#!\[(?:no_std|[^\n]*unused_crate_dependencies[^\n]*|cfg_attr\([^\n]*,\s*no_std\)|doc\s*=\s*include_str!\("[^"]*README[^"]*"\))\]\n', re.M)
MACRO = re.compile(r"#\[macro_export\]\s*(?:#\[[^\]]*\]\s*)*macro_rules!\s*(\w+)")
MANIFEST = re.compile(r'(concat!\(\s*env!\("CARGO_MANIFEST_DIR"\)\s*,\s*)"/([^"]+)"')
INCLUDE = re.compile(r'(include_(?:bytes|str)!\(\s*)"(\.{1,2}/[^"]+)"')


def load(project):
    projects = json.load(open(os.path.join(HERE, "projects.json")))
    if project not in projects or project == "_":
        raise SystemExit(f"unknown project {project}; known: {', '.join(k for k in projects if k != '_')}")
    return projects[project]


def git(repo, *args):
    return subprocess.run(["git", "-C", repo, *args], capture_output=True, check=True).stdout


def files(repo, rev, prefix):
    listed = git(repo, "ls-tree", "-r", "--name-only", rev, prefix + "/").decode()
    return [line for line in listed.split("\n") if line]


def show(repo, rev, path):
    # A partial clone fetches blobs on demand, and a fetch can fail now and then: try again.
    for _ in range(5):
        done = subprocess.run(["git", "-C", repo, "show", f"{rev}:{path}"], capture_output=True)
        if done.returncode == 0:
            return done.stdout
    raise SystemExit(f"cannot read {rev}:{path}: {done.stderr.decode()}")


def destination(source, rest):
    """Where <source path>/<rest> lands in our crate, and whether it is test code; None to skip."""
    # A module may sit inside another (third_party::hayro): a folder in src/, a prefix in tests/.
    folder, prefix = source["module"].replace("::", "/"), source["module"].replace("::", "_")
    if rest.startswith("src/"):
        dest = f"src/{folder}/" + rest[len("src/"):]
        if dest == f"src/{folder}/lib.rs":
            dest = f"src/{folder}/mod.rs"
        return dest, False
    if not source.get("tests", True):
        return None
    if rest.startswith("tests/it/"):
        return f"tests/{prefix}_it/" + rest[len("tests/it/"):], True
    if rest.startswith("tests/"):
        return f"tests/{prefix}_" + rest[len("tests/"):], True
    return None


def place(sources, path):
    """Where a file of their repository lands in ours: inside a source, or under assets/."""
    for source in sources:
        if path.startswith(source["path"] + "/"):
            found = destination(source, path[len(source["path"]) + 1:])
            if found:
                return found[0], False
    return "assets/" + path, True


def exists(repo, rev, path):
    return subprocess.run(["git", "-C", repo, "cat-file", "-e", f"{rev}:{path}"], capture_output=True).returncode == 0


TOKEN = re.compile(r'\s*(?:([A-Za-z_][\w]*)|("(?:[^"\\]|\\.)*")|([(),=]))')


def parse_cfg(text, at):
    """One cfg predicate from `at`: (tree, end). A tree is ("all"|"any", [trees]), ("not", tree),
    or ("leaf", text) for `name` and `name = "value"`."""
    m = TOKEN.match(text, at)
    name, at = m.group(1), m.end()
    nxt = TOKEN.match(text, at)
    if name in ("all", "any", "not") and nxt and nxt.group(3) == "(":
        at, parts = nxt.end(), []
        while True:
            close = TOKEN.match(text, at)
            if close and close.group(3) == ")":
                at = close.end()
                break
            part, at = parse_cfg(text, at)
            parts.append(part)
            comma = TOKEN.match(text, at)
            if comma and comma.group(3) == ",":
                at = comma.end()
        return ((name, parts) if name != "not" else ("not", parts[0])), at
    if nxt and nxt.group(3) == "=":
        value = TOKEN.match(text, nxt.end())
        return ("leaf", f"{name} = {value.group(2)}"), value.end()
    return ("leaf", name), at


def fold(tree):
    """True, False, or a predicate's text: `all()` and `any()` folded away."""
    kind = tree[0]
    if kind == "leaf":
        return tree[1]
    if kind == "not":
        inner = fold(tree[1])
        return (not inner) if isinstance(inner, bool) else f"not({inner})"
    unit, zero = (True, False) if kind == "all" else (False, True)
    rest = []
    for part in tree[1]:
        value = fold(part)
        if value is zero:
            return zero
        if value is not unit:
            rest.append(value)
    if not rest:
        return unit
    return rest[0] if len(rest) == 1 else f"{kind}({', '.join(rest)})"


def simplify_cfgs(text):
    """Folds the `all()`/`any()` the feature rules leave in cfg conditions, which would be
    conditions that are not one: `#[cfg(<true>)]` goes, `cfg_attr(<true>, a)` becomes `#[a]`."""
    out, at = [], 0
    pattern = re.compile(r"#(!?)\[cfg(_attr)?\(|\bcfg!\(")
    while True:
        m = pattern.search(text, at)
        if not m:
            out.append(text[at:])
            return "".join(out)
        tree, end = parse_cfg(text, m.end())
        if "all()" not in text[m.end():end] and "any()" not in text[m.end():end]:
            out.append(text[at:end])
            at = end
            continue
        value = fold(tree)
        out.append(text[at:m.start()])
        if m.group(0).startswith("cfg!"):
            out.append(("true" if value else "false") if isinstance(value, bool) else f"cfg!({value})")
            at = TOKEN.match(text, end).end()
        elif m.group(2):  # cfg_attr(pred, attrs)]
            comma = TOKEN.match(text, end)
            close = text.index(")]", comma.end())
            attrs = text[comma.end():close].strip()
            if value is True:
                out.append(f"#{m.group(1)}[{attrs}]")
            elif value is not False:
                out.append(f"#{m.group(1)}[cfg_attr({value}, {attrs})]")
            at = close + 2
            if value is False:
                at = len(text) - len(text[at:].lstrip())
        else:
            close = TOKEN.match(text, end)  # the `)` before `]`
            after = close.end() + 1
            if value is True:
                after = len(text) - len(text[after:].lstrip())
            else:
                out.append(f"#{m.group(1)}[cfg({'any()' if value is False else value})]")
            at = after


def doc_examples(text, source, sources, lib):
    """Code blocks in doc comments are tests of their own, built against the crate from outside:
    the crate names in them become `<lib>::<module>`, as in tests/. They are set aside under a
    placeholder so the rules for src/ leave them alone; returns the text and the lines to put back."""
    lines, kept, fence, block = text.split("\n"), [], None, False
    for i, line in enumerate(lines):
        # Line comments (`///`, `//!`), or the lines of a block one (`/*!`, `/**` up to `*/`).
        if block:
            doc = re.match(r"^(\s*\*?\s?)(.*)$", line)
            block = "*/" not in line
        elif re.match(r"^\s*/\*[*!]", line):
            block = "*/" not in line
            fence = None
            continue
        else:
            doc = re.match(r"^(\s*(?:///|//!)\s?)(.*)$", line)
        if not doc:
            fence = None
            continue
        opener = re.match(r"^(`{3,})(.*)$", doc.group(2).strip())
        if opener and fence is None:
            fence = opener.group(1)
            if source.get("unit_tests") is False and opener.group(2).strip() in ("", "rust", "no_run", "should_panic", "compile_fail"):
                # Never run where it came from either.
                lines[i] = doc.group(1) + opener.group(1) + "ignore"
            continue
        if opener and doc.group(2).strip() == fence:
            fence = None
            continue
        if fence is not None:
            for other in sources:
                lines[i] = re.sub(rf"(?<![\w\"'/:-]){other['ident']}(?=\s*::|\s*;|\s*,|\s*}}|\s+as\b)", f"{lib}::{other['module']}", lines[i])
            kept.append(lines[i])
            lines[i] = f"\x00{len(kept) - 1}\x00"
    return "\n".join(lines), kept


def rewrite(text, source, sources, lib, in_tests, repo_path, dest, assets, present):
    module = source["module"]
    text, examples = doc_examples(text, source, sources, lib)
    if not in_tests:
        # The crate's own root is now its module.
        text = re.sub(r"\bcrate::", f"crate::{module}::", text)
        text = re.sub(r"(?<![\w:])crate\s*(?=[,}])", f"crate::{module}", text)
    for other in sources:
        target = f"{lib}::{other['module']}" if in_tests else f"crate::{other['module']}"
        if other.get("precise"):
            # A name that is also a word (hayro, lopdf): only where it is a path or a `use`.
            text = re.sub(rf"(?<![\w\"'/:-]){other['ident']}(?=\s*::|\s*;|\s*,|\s*}}|\s+as\b)", target, text)
        else:
            text = re.sub(rf"\b{other['ident']}\b", target, text)
    for name, value in source.get("features", {}).items():
        spelled = rf'feature\s*=\s*"{re.escape(name)}"'
        if value is True:
            # An attribute that only asked for the feature goes; inside a longer condition it holds.
            text = re.sub(rf"#\[cfg\(\s*{spelled}\s*\)\]\s*", "", text)
            text = re.sub(spelled, "all()", text)
        elif value is False:
            text = re.sub(spelled, "any()", text)
        else:
            text = re.sub(spelled, f'feature = "{value}"', text)

    def include(match):
        theirs = posixpath.normpath(posixpath.join(posixpath.dirname(repo_path), match.group(2)))
        ours, is_asset = place(sources, theirs)
        if is_asset:
            if not present(theirs):
                # Behind a feature that is off, a file of the upstream repository they vendored.
                print(f"left as it was, not in their repository: {theirs} ({repo_path})")
                return match.group(0)
            assets.add(theirs)
        return match.group(1) + '"' + posixpath.relpath(ours, posixpath.dirname(dest)) + '"'

    if dest == f"src/{source['module'].replace('::', '/')}/mod.rs":
        # What only a crate root may say: as a module these would warn, and the README is theirs.
        text = CRATE_ONLY.sub("", text)
    text = INCLUDE.sub(include, text)

    def manifest(match):
        # A path from the source's manifest directory, at run time: from our crate's, the same way.
        theirs = posixpath.normpath(posixpath.join(source["path"], match.group(2)))
        ours, is_asset = place(sources, theirs)
        if is_asset:
            if not present(theirs):
                return match.group(0)
            assets.add(theirs)
        return match.group(1) + '"/' + ours + '"'

    text = MANIFEST.sub(manifest, text)
    if source.get("unit_tests") is False:
        # Their own unit tests, which the project never ran (a vendored crate outside its
        # workspace), want dev-dependencies of their own: compiled out, like a feature that is off.
        text = re.sub(r"\b(cfg|cfg_attr|not|any|all)\(\s*test\s*(?=[,)])", r"\1(any()", text)
        text = re.sub(r"(?m)^(\s*)#\[(?:tokio::)?test\]", r"\1#[cfg(any())]", text)
    text = simplify_cfgs(text)
    for rule in source.get("replace", []):
        text = re.sub(rule["from"], rule["to"], text)
    return re.sub("\x00(\\d+)\x00", lambda m: examples[int(m.group(1))], text)


def main():
    if len(sys.argv) != 5:
        raise SystemExit(__doc__)
    project, repo, rev, out = sys.argv[1:]
    config = load(project)
    sources, lib = config["sources"], config["lib"]
    written, assets = 0, set()
    for source in sources:
        listed = files(repo, rev, source["path"])
        exported = sorted({name for path in listed if path.endswith(".rs") and "/src/" in path for name in MACRO.findall(show(repo, rev, path).decode())})
        for path in listed:
            found = destination(source, path[len(source["path"]) + 1:])
            if not found:
                continue
            dest, in_tests = found
            data = show(repo, rev, path)
            if path.endswith(".rs"):
                data = rewrite(data.decode(), source, sources, lib, in_tests, path, dest, assets, lambda p: exists(repo, rev, p)).encode()
            if exported and dest == f"src/{source['module'].replace('::', '/')}/mod.rs":
                # #[macro_export] puts a macro at the root of our crate; the crate had it at its own.
                again = "// RavenRS (convert.py): the macros the crate exported, where it had them.\n#[doc(hidden)]\npub use crate::{" + ", ".join(exported) + "};\n"
                text = data.decode()
                tests = re.search(r"^#\[cfg\(test\)\]\s*\nmod tests\b", text, re.M)
                # Before the tests module, if there is one: items after it are a lint of their own.
                data = (text[:tests.start()] + again + "\n" + text[tests.start():] if tests else text + "\n" + again).encode()
            full = os.path.join(out, dest)
            os.makedirs(os.path.dirname(full), exist_ok=True)
            open(full, "wb").write(data)
            written += 1
    for path in sorted(assets):
        full = os.path.join(out, "assets", path)
        os.makedirs(os.path.dirname(full), exist_ok=True)
        open(full, "wb").write(show(repo, rev, path))
    rs = [os.path.join(r, x) for r, _, xs in os.walk(out) for x in xs if x.endswith(".rs")]
    style = os.path.join(ROOT, config["crate"], "rustfmt.toml")
    for chunk in range(0, len(rs), 200):
        subprocess.run(["rustfmt", "--edition", "2024", "--config-path", style, *rs[chunk:chunk + 200]], capture_output=True)
    print(f"{written} files and {len(assets)} assets from {project} {rev}")


main()
