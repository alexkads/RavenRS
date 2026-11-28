#!/usr/bin/env python3
"""Keeps the design system that RavenComponentBook exports in the repository (#1000).

`docs/design-system/export/` holds what `raven-component-book --claude-design` writes: the index
`project/design-system.json`, the guide, the tokens of `ravenrs-style`, the fonts, the cover and,
per component and pattern, its card, its preview and its JSON with the anatomy of every story.
Whoever needs the design system (a person, an agent, the Claude Design project) reads it from git
instead of building RCB and measuring every component again.

Run it after changing a component, a story or a token: cargo xtask pr fails while the copy is
out of date (`--check`, which runs RCB's `--verify` and ignores the time and the commit the index
and the tokens carry). `--self-test` checks how the report of `--verify` is read."""
import pathlib
import shutil
import subprocess
import sys

root = pathlib.Path(__file__).resolve().parent.parent
folder = root / "docs" / "design-system" / "export"
# The ComponentBook is a workspace of its own (#1085): cargo reaches it through its manifest.
rcb = ["cargo", "run", "-q", "--manifest-path", "tools/raven-component-book/Cargo.toml", "--"]


def drift(report):
    """The paths `--verify` reports, as (what, path): `mudou`, `falta` or `sobra`."""
    found = []
    for line in report.splitlines():
        what, sep, path = line.partition(": ")
        if sep and what in ("mudou", "falta", "sobra") and path.strip():
            found.append((what, path.strip()))
    return found


def self_test():
    report = "mudou: project/components/Card/Card.json\nfalta: project/components/Avatar/README.md\n" \
        "sobra: project/components/Old/Old.json\nCompiling raven-component-book\n"
    assert drift(report) == [
        ("mudou", "project/components/Card/Card.json"),
        ("falta", "project/components/Avatar/README.md"),
        ("sobra", "project/components/Old/Old.json"),
    ], drift(report)
    assert drift("o design system publicado é exatamente o que o RCB gera\n") == []
    assert drift("mudou: \n") == [], "a line without a path is not a path"
    print("gen-design-system self-test: 3 cases")


def check():
    if not (folder / "project" / "design-system.json").is_file():
        sys.exit("missing, run scripts/gen-design-system.py: " + str(folder.relative_to(root)))
    run = subprocess.run(rcb + ["--verify", str(folder)], cwd=root, capture_output=True, text=True)
    found = drift(run.stdout)
    if run.returncode == 0 and not found:
        return
    if run.returncode not in (0, 1) or not found:
        sys.stderr.write(run.stdout + run.stderr)
        sys.exit("could not compare the design system with what RCB writes")
    for what, path in found:
        print(f"{what}: {folder.relative_to(root)}/{path}")
    sys.exit("out of date, run scripts/gen-design-system.py")


def generate():
    sha = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=root, capture_output=True,
                         text=True, check=True).stdout.strip()
    # From nothing, so a component that left takes its files with it.
    shutil.rmtree(folder / "project", ignore_errors=True)
    (folder / "files.txt").unlink(missing_ok=True)
    subprocess.run(rcb + ["--claude-design", str(folder), "--sha", sha], cwd=root, check=True)
    files = (folder / "files.txt").read_text().split()
    print(f"{len(files)} files in {folder.relative_to(root)}, from {sha}")


if "--self-test" in sys.argv:
    self_test()
elif "--check" in sys.argv:
    check()
else:
    generate()
