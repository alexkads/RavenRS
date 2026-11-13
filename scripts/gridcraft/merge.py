#!/usr/bin/env python3
"""Brings a newer GridCraft into ravenrs-sheet, keeping our own changes: a three-way merge per file.

    scripts/gridcraft/merge.py <old converted> <new converted> [crate dir]

<old converted> and <new converted> are scripts/gridcraft/convert.py run on the revision in
NOTICE and on the one to bring. For every file under src/ and tests/:

- unchanged here: takes the new one;
- unchanged there: keeps ours;
- changed on both sides: `git merge-file`, leaving conflict markers where the changes overlap;
- new there: added; removed there: removed, unless we changed it;
- only here (lib.rs, files of our own): kept.

It prints what it did and the files left with conflicts, and exits with 1 if there are any.
"""
import filecmp
import os
import shutil
import subprocess
import sys

old, new = sys.argv[1], sys.argv[2]
crate = sys.argv[3] if len(sys.argv) > 3 else os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "crates", "ravenrs-sheet")


def tree(root):
    found = set()
    for top in ("src", "tests"):
        for directory, _, names in os.walk(os.path.join(root, top)):
            for name in names:
                found.add(os.path.relpath(os.path.join(directory, name), root))
    return found


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
        done = subprocess.run(["git", "merge-file", "-L", "ravenrs", "-L", "gridcraft-old", "-L", "gridcraft-new", here, base, theirs])
        if done.returncode == 0:
            counts["merged"] += 1
        else:
            conflicts.append(path)

# What the removals emptied goes too.
for top in ("src", "tests"):
    for directory, subdirs, names in sorted(os.walk(os.path.join(crate, top)), reverse=True):
        if not subdirs and not names:
            os.rmdir(directory)

for what, n in counts.items():
    print(f"{n:5} {what}")
for path in removed_but_changed:
    print(f"removed there, changed here (decide by hand): {path}")
for path in conflicts:
    print(f"conflict: {path}")
sys.exit(1 if conflicts or removed_but_changed else 0)
