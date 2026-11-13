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
  assets/<their path>, copied too;
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


def rewrite(text, source, sources, lib, in_tests, repo_path, dest, assets):
    module = source["module"]
    if not in_tests:
        # The crate's own root is now its module.
        text = re.sub(r"\bcrate::", f"crate::{module}::", text)
        text = re.sub(r"(?<![\w:])crate\s*(?=[,}])", f"crate::{module}", text)
    for other in sources:
        target = f"{lib}::{other['module']}" if in_tests else f"crate::{other['module']}"
        if other.get("precise"):
            # A name that is also a word (hayro, lopdf): only where it is a path or a `use`.
            text = re.sub(rf"(?<![\w\"'/.:-]){other['ident']}(?=\s*::|\s*;|\s*,|\s*}}|\s+as\b)", target, text)
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
            assets.add(theirs)
        return match.group(1) + '"' + posixpath.relpath(ours, posixpath.dirname(dest)) + '"'

    text = INCLUDE.sub(include, text)
    for rule in source.get("replace", []):
        text = re.sub(rule["from"], rule["to"], text)
    return text


def main():
    if len(sys.argv) != 5:
        raise SystemExit(__doc__)
    project, repo, rev, out = sys.argv[1:]
    config = load(project)
    sources, lib = config["sources"], config["lib"]
    written, assets = 0, set()
    for source in sources:
        for path in files(repo, rev, source["path"]):
            found = destination(source, path[len(source["path"]) + 1:])
            if not found:
                continue
            dest, in_tests = found
            data = show(repo, rev, path)
            if path.endswith(".rs"):
                data = rewrite(data.decode(), source, sources, lib, in_tests, path, dest, assets).encode()
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
