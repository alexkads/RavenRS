#!/usr/bin/env python3
"""Brings a newer revision of a storytold project into its RavenRS crate, keeping our own changes:
a three-way merge per file.

    scripts/storytold/merge.py <project> <old converted> <new converted> [crate dir]

<old converted> and <new converted> are convert.py run on the revision in the crate's NOTICE and
on the one to bring; the crate is the one projects.json names, unless given. For every file under
src/, tests/ and assets/:

- unchanged here: takes the new one;
- unchanged there: keeps ours;
- changed on both sides: `git merge-file`, leaving conflict markers where the changes overlap;
- new there: added; removed there: removed, unless we changed it;
- only here (lib.rs, files of our own): kept.

It prints what it did and the files left with conflicts, and exits with 1 if there are any.
"""
import filecmp
import json
import os
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
TOPS = ("src", "tests", "assets")


def tree(root):
    found = set()
    for top in TOPS:
        for directory, _, names in os.walk(os.path.join(root, top)):
            for name in names:
                found.add(os.path.relpath(os.path.join(directory, name), root))
    return found


def main():
    if len(sys.argv) not in (4, 5):
        raise SystemExit(__doc__)
    project, old, new = sys.argv[1:4]
    projects = json.load(open(os.path.join(HERE, "projects.json")))
    if project not in projects or project == "_":
        raise SystemExit(f"unknown project {project}")
    crate = sys.argv[4] if len(sys.argv) > 4 else os.path.join(HERE, "..", "..", projects[project]["crate"])

    in_old, in_new, ours = tree(old), tree(new), tree(crate)
    counts = {"took theirs": 0, "kept ours": 0, "merged": 0, "added": 0, "removed": 0, "kept, ours only": 0}
    conflicts, removed_but_changed = [], []
    for path in sorted(in_old | in_new | ours):
        here, base, theirs = (os.path.join(crate, path), os.path.join(old, path), os.path.join(new, path))
        if path in ours and path not in in_old and path not in in_new:
            counts["kept, ours only"] += 1
        elif path in in_new and path not in in_old:
            if path in ours and not filecmp.cmp(here, theirs, shallow=False):
                conflicts.append(path + " (added on both sides)")
                continue
            os.makedirs(os.path.dirname(here), exist_ok=True)
            shutil.copyfile(theirs, here)
            counts["added"] += 1
        elif path in in_old and path not in in_new:
            if path in ours and not filecmp.cmp(here, base, shallow=False):
                removed_but_changed.append(path)
            elif path in ours:
                os.remove(here)
                counts["removed"] += 1
        elif path not in ours:
            continue  # removed here on purpose
        elif filecmp.cmp(here, base, shallow=False):
            shutil.copyfile(theirs, here)
            counts["took theirs"] += 1
        elif filecmp.cmp(base, theirs, shallow=False):
            counts["kept ours"] += 1
        else:
            labels = ["-L", "ravenrs", "-L", f"{project}-old", "-L", f"{project}-new"]
            done = subprocess.run(["git", "merge-file", *labels, here, base, theirs])
            if done.returncode == 0:
                counts["merged"] += 1
            else:
                conflicts.append(path)

    for what, n in counts.items():
        print(f"{n:5} {what}")
    for path in removed_but_changed:
        print(f"removed there, changed here (decide by hand): {path}")
    for path in conflicts:
        print(f"conflict: {path}")
    sys.exit(1 if conflicts or removed_but_changed else 0)


main()
