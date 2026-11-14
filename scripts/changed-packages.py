#!/usr/bin/env python3
"""Prints the workspace packages a change reaches, against the base (origin/master, or CHECK_BASE),
committed or not.

    scripts/changed-packages.py           # every package reached, one name per line; ALL or nothing
    scripts/changed-packages.py --shell   # the sets scripts/check.sh uses, as shell assignments

The sets:

- touched: packages whose own files changed, plus those a dependency change reaches (an external
  crate that moved in Cargo.lock, a [workspace.dependencies] entry that changed);
- tested: touched and the packages that depend on them directly; the quick check runs their tests
  (and documented, those of them with a library, their doctests);
- reached: touched and every package that depends on them, at any depth; the quick check lints
  them, which also proves they still compile;
- lint: whether ravenrs-lint has something new to read (its own crate, the examples it checks, its
  configuration in a Cargo.toml).

The root Cargo.toml and Cargo.lock are compared section by section and package by package, so a new
crate or a version bump does not mean the whole workspace. A change it cannot place (profiles,
lints, the edition, .cargo/, the toolchain, clippy.toml) still means ALL.
"""
import json
import os
import pathlib
import re
import subprocess
import sys

root = pathlib.Path(__file__).resolve().parent.parent
# What every package builds with: a change here reaches all of them.
SHARED = (".cargo/", "rust-toolchain", "rust-toolchain.toml", "clippy.toml")
# Read only by steps that cover the whole workspace anyway (rustfmt, cargo-deny in --full).
IGNORED = ("rustfmt.toml", "deny.toml")
# What ravenrs-lint reads in the quick check: its crate and the examples it runs on.
LINTED = ("ravenrs-lint", "family-finance", "component-gallery")


def git(*args, check=True):
    result = subprocess.run(["git", *args], cwd=root, capture_output=True, text=True)
    if check and result.returncode != 0:
        raise subprocess.CalledProcessError(result.returncode, args, result.stdout, result.stderr)
    return result.stdout if result.returncode == 0 else None


class Everything(Exception):
    """A change that cannot be narrowed down to some packages."""


# --- Cargo.toml, the root one ------------------------------------------------------------------
# Python 3.9 has no tomllib: this reads the subset the root manifest uses (sections, and keys whose
# value may span lines while a bracket or brace is open). Values are compared as normalised text.


def toml_sections(text):
    sections, name, key, value, depth = {}, "", None, "", 0
    for line in text.splitlines():
        stripped = re.sub(r"\s+#.*$", "", line).strip() if '"' not in line else line.strip()
        if key is not None:
            value += " " + stripped
            depth += stripped.count("[") + stripped.count("{") - stripped.count("]") - stripped.count("}")
            if depth <= 0:
                sections.setdefault(name, {})[key] = " ".join(value.split())
                key = None
            continue
        if not stripped or stripped.startswith("#"):
            continue
        header = re.fullmatch(r"\[\[?([^\]]+)\]\]?", stripped)
        if header:
            name = header.group(1).strip()
            sections.setdefault(name, {})
            continue
        match = re.match(r'("?[^=]+?"?)\s*=\s*(.*)$', stripped)
        if not match:
            raise Everything(f"Cargo.toml: cannot read {stripped!r}")
        key, value = match.group(1).strip('"'), match.group(2)
        depth = value.count("[") + value.count("{") - value.count("]") - value.count("}")
        if depth <= 0:
            sections.setdefault(name, {})[key] = " ".join(value.split())
            key = None
    return sections


def without_version(value):
    return re.sub(r'version\s*=\s*"[^"]*"\s*,?\s*', "", value)


def manifest_changes(old, new):
    """What a change to the root Cargo.toml reaches: (dependency names, version bumped, lint)."""
    old, new = toml_sections(old), toml_sections(new)
    deps, bumped, lint = set(), False, False
    for section in set(old) | set(new):
        before, after = old.get(section, {}), new.get(section, {})
        if before == after:
            continue
        if section == "workspace":
            # members/exclude: a new crate arrives with its own files, which the diff already has.
            if {k for k in set(before) | set(after) if before.get(k) != after.get(k)} - {"members", "exclude"}:
                raise Everything("Cargo.toml: [workspace] changed")
        elif section == "workspace.package":
            changed = {k for k in set(before) | set(after) if before.get(k) != after.get(k)}
            if changed - {"version"}:
                raise Everything("Cargo.toml: [workspace.package] changed")
            bumped = True
        elif section == "workspace.dependencies":
            for name in set(before) | set(after):
                a, b = before.get(name), after.get(name)
                if a == b:
                    continue
                # An internal crate whose version alone moved builds the same code.
                if a and b and "path" in b and without_version(a) == without_version(b):
                    bumped = True
                    continue
                deps.add(name)
        elif section.startswith("workspace.metadata"):
            lint = True
        else:
            raise Everything(f"Cargo.toml: [{section}] changed")
    return deps, bumped, lint


# --- Cargo.lock ----------------------------------------------------------------------------------


def lock_packages(text):
    """Cargo.lock as {key: (checksum, dependency names)}; key is the name for workspace members
    (no source) and (name, version, source) for everything else."""
    packages = {}
    for block in text.split("[[package]]")[1:]:
        fields = dict(re.findall(r'^(\w+) = "([^"]*)"', block, re.M))
        deps = re.search(r"dependencies = \[(.*?)\]", block, re.S)
        names = sorted(re.findall(r'"([^"]+)"', deps.group(1))) if deps else []
        if "source" in fields:
            packages[(fields["name"], fields["version"], fields["source"])] = (fields.get("checksum"), tuple(names))
        else:
            # Members: compare the names they depend on; a moved external version shows up in
            # that crate's own entry, and the version of the member itself is a bump.
            packages[fields["name"]] = (None, tuple(sorted({d.split(" ")[0] for d in names})))
    return packages


def lock_changes(old, new, members):
    """The workspace members a Cargo.lock change reaches through external crates."""
    old, new = lock_packages(old), lock_packages(new)
    changed = {key for key, value in new.items() if old.get(key) != value}
    touched = {key for key in changed if isinstance(key, str)}
    # Walk up from the external crates that changed, through other external crates, to members.
    users = {}
    for key, (_, deps) in new.items():
        for dep in deps:
            users.setdefault(dep.split(" ")[0], set()).add(key)
    waiting = [key for key in changed if not isinstance(key, str)]
    seen = set()
    while waiting:
        key = waiting.pop()
        if key in seen:
            continue
        seen.add(key)
        for user in users.get(key[0], ()):
            if isinstance(user, str):
                touched.add(user)
            else:
                waiting.append(user)
    return touched & members


# --- the change ----------------------------------------------------------------------------------


def changed_files(fork):
    files = set(git("diff", "--name-only", fork).split("\n")) | set(
        git("ls-files", "--others", "--exclude-standard").split("\n")
    )
    return {f for f in files if f}


def at(revision, path):
    return git("show", f"{revision}:{path}", check=False) or ""


def analyse():
    base = os.environ.get("CHECK_BASE", "origin/master")
    try:
        fork = git("merge-base", "HEAD", base).strip()
    except subprocess.CalledProcessError:
        raise Everything(f"no merge base with {base}")
    files = changed_files(fork)
    shared = sorted(f for f in files for s in SHARED if f == s or f.startswith(s))
    if shared:
        raise Everything(f"{shared[0]} changed")
    meta = json.loads(
        subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--no-deps", "--offline"],
            cwd=root, capture_output=True, text=True, check=True,
        ).stdout
    )
    packages = {p["name"]: pathlib.Path(p["manifest_path"]).parent.relative_to(root) for p in meta["packages"]}
    members = set(packages)
    # Deepest folder first, so a file goes to the package it is really in.
    by_folder = sorted(packages.items(), key=lambda item: len(item[1].parts), reverse=True)
    touched, lint = set(), False
    for file in files:
        if file in ("Cargo.toml", "Cargo.lock") or file in IGNORED:
            continue
        path = pathlib.Path(file)
        for name, folder in by_folder:
            if folder.parts and path.parts[: len(folder.parts)] == folder.parts:
                touched.add(name)
                break
    if "Cargo.toml" in files:
        deps, bumped, manifest_lint = manifest_changes(at(fork, "Cargo.toml"), (root / "Cargo.toml").read_text())
        lint |= manifest_lint
        for package in meta["packages"]:
            if any((d.get("rename") or d["name"]) in deps or d["name"] in deps for d in package["dependencies"]):
                touched.add(package["name"])
        if bumped:
            # The version is compiled in only where the code asks for it.
            for file in git("grep", "-l", "CARGO_PKG_VERSION", "--", "*.rs", check=False).split("\n"):
                path = pathlib.Path(file)
                for name, folder in by_folder:
                    if file and folder.parts and path.parts[: len(folder.parts)] == folder.parts:
                        touched.add(name)
                        break
    if "Cargo.lock" in files:
        touched |= lock_changes(at(fork, "Cargo.lock"), (root / "Cargo.lock").read_text(), members)
    dependents = {name: set() for name in packages}
    for package in meta["packages"]:
        for dependency in package["dependencies"]:
            if dependency.get("path") and dependency["name"] in dependents:
                dependents[dependency["name"]].add(package["name"])
    tested = set(touched)
    for name in touched:
        tested |= dependents[name]
    reached, waiting = set(), list(touched)
    while waiting:
        name = waiting.pop()
        if name in reached:
            continue
        reached.add(name)
        waiting.extend(dependents.get(name, ()))
    lint |= bool(touched & set(LINTED))
    # `cargo test --doc` refuses a package with no library.
    libraries = {
        p["name"] for p in meta["packages"] if any("lib" in t["kind"] or "proc-macro" in t["kind"] for t in p["targets"])
    }
    return touched, tested & libraries, tested, reached, lint


def main():
    shell = "--shell" in sys.argv[1:]
    try:
        touched, documented, tested, reached, lint = analyse()
        everything = False
    except Everything as reason:
        print(f"{reason}: checking everything", file=sys.stderr)
        everything, touched, documented, tested, reached, lint = True, set(), set(), set(), set(), True
    if shell:
        print(f"ALL={int(everything)}")
        print(f'TOUCHED="{" ".join(sorted(touched))}"')
        print(f'TESTED="{" ".join(sorted(tested))}"')
        print(f'DOCUMENTED="{" ".join(sorted(documented))}"')
        print(f'REACHED="{" ".join(sorted(reached))}"')
        print(f"LINT={int(lint)}")
    elif everything:
        print("ALL")
    else:
        for name in sorted(reached):
            print(name)


if __name__ == "__main__":
    main()
