#!/usr/bin/env python3
"""Prints the workspace packages a change reaches: those whose files changed against the base
(origin/master, or CHECK_BASE), committed or not, and every package that depends on them. Prints
ALL when something every package builds with changed, and nothing when no package did.

    scripts/changed-packages.py            # one name per line
"""
import json
import os
import pathlib
import subprocess
import sys

root = pathlib.Path(__file__).resolve().parent.parent
# What every package builds with: a change here reaches all of them.
SHARED = ("Cargo.toml", "Cargo.lock", ".cargo/", "rust-toolchain", "rust-toolchain.toml", "clippy.toml", "rustfmt.toml", "deny.toml")


def git(*args):
    return subprocess.run(["git", *args], cwd=root, capture_output=True, text=True, check=True).stdout.split("\n")


def changed_files():
    base = os.environ.get("CHECK_BASE", "origin/master")
    try:
        fork = git("merge-base", "HEAD", base)[0]
    except subprocess.CalledProcessError:
        print(f"no merge base with {base}: checking everything", file=sys.stderr)
        return None
    files = set(git("diff", "--name-only", fork)) | set(git("ls-files", "--others", "--exclude-standard"))
    return {f for f in files if f}


def main():
    files = changed_files()
    if files is None or any(f == s or f.startswith(s) for f in files for s in SHARED):
        print("ALL")
        return
    meta = json.loads(
        subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--no-deps", "--offline"],
            cwd=root, capture_output=True, text=True, check=True,
        ).stdout
    )
    packages = {p["name"]: pathlib.Path(p["manifest_path"]).parent.relative_to(root) for p in meta["packages"]}
    # Deepest folder first, so a file goes to the package it is really in.
    by_folder = sorted(packages.items(), key=lambda item: len(item[1].parts), reverse=True)
    touched = set()
    for file in files:
        path = pathlib.Path(file)
        for name, folder in by_folder:
            if folder.parts and path.parts[: len(folder.parts)] == folder.parts:
                touched.add(name)
                break
    dependents = {name: set() for name in packages}
    for package in meta["packages"]:
        for dependency in package["dependencies"]:
            if dependency.get("path") and dependency["name"] in dependents:
                dependents[dependency["name"]].add(package["name"])
    reached, waiting = set(), list(touched)
    while waiting:
        name = waiting.pop()
        if name in reached:
            continue
        reached.add(name)
        waiting.extend(dependents.get(name, ()))
    for name in sorted(reached):
        print(name)


if __name__ == "__main__":
    main()
