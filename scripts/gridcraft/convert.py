#!/usr/bin/env python3
"""Turns GridCraft's crates, and the craft-libs crates they use, into the module layout of
ravenrs-sheet, at one revision of each.

    scripts/gridcraft/convert.py <gridcraft checkout> <revision> <out dir> \\
        [--craft-libs <craft-libs checkout> <revision>]

Run against the revisions in crates/ravenrs-sheet/NOTICE, it reproduces the crate but for our
own changes; that is how the conversion is checked before newer revisions are merged in. See the
ravenrs-gridcraft skill.

- GridCraft: crates/<c>/src/** becomes src/<m>/** (core is named base), lib.rs becomes mod.rs;
  crates/<c>/tests/it/** becomes tests/<m>_it/**, and crates/<c>/tests/<f> becomes tests/<m>_<f>.
- craft-libs: crates/<c>/src/** becomes src/craft/<c>/**, its README goes along for the docs
  that include it, and crates/<c>/tests/<f> becomes tests/craft_<c>_<f> (a `common` folder of
  test helpers becomes tests/craft_<c>_common/).
- Paths to the crates (gridcraft_X, craft_X) become paths to the modules, and `crate::` inside a
  crate becomes the path to its module.
"""
import os
import re
import subprocess
import sys

args = sys.argv[1:]
craft = None
if "--craft-libs" in args:
    i = args.index("--craft-libs")
    craft = (args[i + 1], args[i + 2])
    args = args[:i] + args[i + 3:]
repo, rev, out = args

GRIDCRAFT = ["core", "numfmt", "formula", "functions", "math", "model", "calc", "xlsx", "chart", "pdf", "engine"]
# What GridCraft's crates use from craft-libs, with what those use in turn.
CRAFT = ["cfb", "csv", "offcrypto", "opc", "sheet", "xls", "xml", "zip"]
MODULE = {**{c: ("base" if c == "core" else c) for c in GRIDCRAFT}}


def files(checkout, rev, prefix):
    listed = subprocess.run(["git", "-C", checkout, "ls-tree", "-r", "--name-only", rev, prefix], capture_output=True, text=True, check=True).stdout
    return [line for line in listed.split("\n") if line]


def show(checkout, rev, path):
    # A partial clone fetches blobs on demand, and a fetch can fail now and then: try again.
    for _ in range(5):
        done = subprocess.run(["git", "-C", checkout, "show", f"{rev}:{path}"], capture_output=True)
        if done.returncode == 0:
            return done.stdout
    raise SystemExit(f"cannot read {rev}:{path}: {done.stderr.decode()}")


def rewrite(text, own, in_tests):
    """`own` is the module path of the crate the file belongs to, such as `engine` or `craft::opc`."""
    if not in_tests:
        # The crate's own root is now its module.
        text = re.sub(r"\bcrate::", f"crate::{own}::", text)
        text = re.sub(r"(?<![\w:])crate\s*(?=[,}])", f"crate::{own}", text)
    root = "ravenrs_sheet::" if in_tests else "crate::"
    for other in GRIDCRAFT:
        text = re.sub(rf"\bgridcraft_{other}\b", root + MODULE[other], text)
    for other in CRAFT:
        text = re.sub(rf"\bcraft_{other}\b", root + "craft::" + other, text)
    text = re.sub(r"\bcraft_media_testkit\b", "crate::craft::media_testkit", text)
    return text


def write(dest, data):
    full = os.path.join(out, dest)
    os.makedirs(os.path.dirname(full), exist_ok=True)
    open(full, "wb").write(data)


written = 0
for crate in GRIDCRAFT:
    m = MODULE[crate]
    for path in files(repo, rev, f"crates/{crate}/"):
        rest = path[len(f"crates/{crate}/"):]
        if rest.startswith("src/"):
            dest = f"src/{m}/" + rest[len("src/"):]
            if dest == f"src/{m}/lib.rs":
                dest = f"src/{m}/mod.rs"
            in_tests = False
        elif rest.startswith("tests/it/"):
            dest, in_tests = f"tests/{m}_it/" + rest[len("tests/it/"):], True
        elif rest.startswith("tests/") and rest.endswith(".rs") and rest.count("/") == 1:
            dest, in_tests = f"tests/{m}_" + rest[len("tests/"):], True
        elif rest.startswith("tests/"):
            # Data the tests read by a path relative to tests/, such as golden/.
            dest, in_tests = rest, True
        else:
            continue
        data = show(repo, rev, path)
        if path.endswith(".rs"):
            data = rewrite(data.decode(), m, in_tests).encode()
        write(dest, data)
        written += 1

if craft:
    checkout, craft_rev = craft
    for crate in CRAFT:
        own = f"craft::{crate}"
        for path in files(checkout, craft_rev, f"crates/{crate}/"):
            rest = path[len(f"crates/{crate}/"):]
            if rest.startswith("src/"):
                dest = f"src/craft/{crate}/" + rest[len("src/"):]
                if dest == f"src/craft/{crate}/lib.rs":
                    dest = f"src/craft/{crate}/mod.rs"
                in_tests = False
            elif rest == "README.md":
                dest, in_tests = f"src/craft/{crate}/README.md", False
            elif rest.startswith("tests/common/"):
                dest, in_tests = f"tests/craft_{crate}_common/" + rest[len("tests/common/"):], True
            elif rest.startswith("tests/"):
                dest, in_tests = f"tests/craft_{crate}_" + rest[len("tests/"):], True
            else:
                continue
            data = show(checkout, craft_rev, path)
            if rest == "README.md":
                # Its examples run as doctests of ravenrs-sheet.
                data = rewrite(data.decode(), own, True).encode()
            elif path.endswith(".rs"):
                text = rewrite(data.decode(), own, in_tests)
                # Examples in the docs run as doctests, outside the crate.
                text = re.sub(r"^(\s*//[/!].*)$", lambda m: m.group(1).replace("crate::craft::", "ravenrs_sheet::craft::"), text, flags=re.M)
                # The README now sits beside the module, and test helpers in a folder of their own.
                text = text.replace('include_str!("../README.md")', 'include_str!("README.md")')
                text = re.sub(r"^mod common;", f'#[path = "craft_{crate}_common/mod.rs"]\nmod common;', text, flags=re.M)
                data = text.encode()
            write(dest, data)
            written += 1
    # The one helper the craft crates' tests take from media-testkit: a fuzzing loop.
    hostile = show(checkout, craft_rev, "crates/media-testkit/src/hostile.rs").decode()
    write("src/craft/media_testkit/hostile.rs", hostile.encode())
    write("src/craft/media_testkit/mod.rs", b"//! The fuzzing loop of craft-libs' media-testkit, for the tests of the modules beside it.\n\npub mod hostile;\n")
    written += 2
    os.makedirs(os.path.join(out, "src/craft"), exist_ok=True)
    open(os.path.join(out, "src/craft/mod.rs"), "w").write(
        "//! The crates of craft-libs that GridCraft's file formats use, as modules (see `NOTICE`).\n\n"
        # In the order rustfmt keeps them: by name, the test-only helper among them.
        + "".join(f"pub mod {c};\n" if c != "media_testkit" else "#[cfg(test)]\nmod media_testkit;\n" for c in sorted(CRAFT + ["media_testkit"]))
    )

# The paths changed length, so lines break elsewhere: format as ravenrs-sheet does.
rs = [os.path.join(r, x) for r, _, xs in os.walk(out) for x in xs if x.endswith(".rs")]
config = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "crates", "ravenrs-sheet", "rustfmt.toml")
for chunk in range(0, len(rs), 200):
    subprocess.run(["rustfmt", "--edition", "2024", "--config-path", config, *rs[chunk:chunk + 200]], capture_output=True)
print(f"{written} files from {rev}" + (f" and craft-libs {craft[1]}" if craft else ""))
