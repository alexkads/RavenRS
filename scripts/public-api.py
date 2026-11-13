#!/usr/bin/env python3
"""Lists the public API of a crate by the paths its users write, from rustdoc's JSON.

    scripts/public-api.py                    writes crates/<crate>/public-api.txt
    scripts/public-api.py --check            fails, showing the difference, if it changed
    scripts/public-api.py -p ravenrs-widgets picks the crate (ravenrs-widgets by default)

Each line is one thing a user can name: a module, a type with its kind, a field, a variant, a
method, a constant, a function, a re-export of another crate's item, or a trait a type
implements. Moving an item between modules or crates keeps the file the same as long as it is
re-exported where it was; re-exporting it anywhere else changes a line, and the check fails.

rustdoc writes JSON only on nightly (`rustup toolchain install nightly`). Types are named, not
spelled out: a changed signature is caught by the compiler and by the playground projects."""

import argparse
import difflib
import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
KINDS = {
    "struct": "struct",
    "enum": "enum",
    "union": "union",
    "trait": "trait",
    "function": "fn",
    "type_alias": "type",
    "constant": "const",
    "static": "static",
    "macro": "macro",
    "proc_macro": "macro",
    "trait_alias": "trait",
}


def rustdoc_json(package: str) -> dict:
    target = Path(os.environ.get("CARGO_TARGET_DIR") or ROOT / "target")
    command = [
        "cargo", "+nightly", "rustdoc", "-q", "-p", package, "--all-features",
        "--", "-Z", "unstable-options", "--output-format", "json",
    ]
    done = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
    if done.returncode != 0:
        sys.exit(f"rustdoc failed (is the nightly toolchain installed?):\n{done.stderr[-4000:]}")
    return json.loads((target / "doc" / f"{package.replace('-', '_')}.json").read_text())


def type_name(ty) -> str:
    """The name of a type as the JSON gives it, without its arguments."""
    if not isinstance(ty, dict):
        return "?"
    if "resolved_path" in ty:
        return ty["resolved_path"]["path"].split("::")[-1]
    if "primitive" in ty:
        return ty["primitive"]
    if "generic" in ty:
        return ty["generic"]
    if "borrowed_ref" in ty:
        return "&" + type_name(ty["borrowed_ref"]["type"])
    if "slice" in ty:
        return "[" + type_name(ty["slice"]) + "]"
    if "tuple" in ty:
        return "(" + ", ".join(type_name(t) for t in ty["tuple"]) + ")"
    return next(iter(ty), "?")


def api(data: dict) -> list[str]:
    index, paths = data["index"], data["paths"]
    lines: set[str] = set()
    seen: set[tuple[str, str]] = set()

    def item(id_):
        return index.get(str(id_))

    def members(path: str, it: dict):
        kind, inner = next(iter(it["inner"].items()))
        if kind == "struct" and isinstance(inner["kind"], dict):
            fields = list(inner["kind"].get("plain", {}).get("fields", []))
            fields += inner["kind"].get("tuple", []) or []
            for field_id in fields:
                field = item(field_id) if field_id is not None else None
                if field and field["visibility"] == "public" and field["name"]:
                    lines.add(f"field {path}::{field['name']}")
        if kind == "enum":
            for variant_id in inner["variants"]:
                variant = item(variant_id)
                if variant:
                    lines.add(f"variant {path}::{variant['name']}")
        if kind == "trait":
            for member_id in inner["items"]:
                member = item(member_id)
                if member and member["name"]:
                    lines.add(f"trait-item {path}::{member['name']}")
        for impl_id in inner.get("impls", []):
            impl = item(impl_id)
            if not impl:
                continue
            body = impl["inner"]["impl"]
            if body["is_synthetic"] or body["blanket_impl"] is not None:
                continue
            if body["trait"] is None:
                for member_id in body["items"]:
                    member = item(member_id)
                    if member and member["visibility"] == "public" and member["name"]:
                        member_kind = KINDS.get(next(iter(member["inner"])), "item")
                        lines.add(f"{'method' if member_kind == 'fn' else member_kind} {path}::{member['name']}")
            else:
                trait = body["trait"]["path"].split("::")[-1]
                # The compiler's own marker for derived equality is no one's API.
                if not trait.startswith("Structural"):
                    lines.add(f"impl {trait} for {path}")

    def visit(id_, path: str):
        it = item(id_)
        if it is None:
            # Re-exported from another crate: named by where it really lives.
            source = paths.get(str(id_))
            origin = "::".join(source["path"]) if source else "?"
            lines.add(f"reexport {path} = {origin}")
            return
        key = (path, str(id_))
        if key in seen:
            return
        seen.add(key)
        kind, inner = next(iter(it["inner"].items()))
        if kind == "module":
            if path:
                lines.add(f"mod {path}")
            for child_id in inner["items"]:
                child = item(child_id)
                if child is None or child["visibility"] != "public":
                    continue
                child_kind, child_inner = next(iter(child["inner"].items()))
                if child_kind == "use":
                    target = child_inner["id"]
                    if child_inner["is_glob"]:
                        glob = item(target) if target is not None else None
                        if glob and "module" in glob["inner"]:
                            for grand_id in glob["inner"]["module"]["items"]:
                                grand = item(grand_id)
                                if grand and grand["visibility"] == "public" and grand["name"]:
                                    visit(grand_id, join(path, grand["name"]))
                        else:
                            lines.add(f"reexport {join(path, '*')} = {child_inner['source']}")
                        continue
                    if target is None:
                        lines.add(f"reexport {join(path, child_inner['name'])} = {child_inner['source']}")
                        continue
                    visit(target, join(path, child_inner["name"]))
                elif child["name"] and child_kind != "impl":
                    visit(child_id, join(path, child["name"]))
            return
        if kind == "use":
            return
        lines.add(f"{KINDS.get(kind, kind)} {path}")
        members(path, it)

    def join(prefix: str, name: str) -> str:
        return f"{prefix}::{name}" if prefix else name

    visit(data["root"], "")
    return sorted(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("-p", "--package", default="ravenrs-widgets")
    parser.add_argument("--check", action="store_true", help="fail if the list changed")
    args = parser.parse_args()
    file = ROOT / "crates" / args.package / "public-api.txt"
    header = [
        f"# The public API of {args.package}, by the paths its users write.",
        "# Written by scripts/public-api.py; `--check` fails when it changes. Regenerate it only",
        "# when the change is meant, and say so in the pull request.",
    ]
    now = header + api(rustdoc_json(args.package))
    if not args.check:
        file.write_text("\n".join(now) + "\n")
        print(f"{file.relative_to(ROOT)}: {len(now) - len(header)} items")
        return
    before = file.read_text().splitlines() if file.exists() else []
    if before == now:
        print(f"{file.relative_to(ROOT)}: unchanged, {len(now) - len(header)} items")
        return
    diff = difflib.unified_diff(before, now, "public-api.txt", "now", lineterm="", n=0)
    sys.exit(
        f"the public API of {args.package} changed:\n"
        + "\n".join(line for line in diff if not line.startswith("@@"))
        + "\nIf this is meant, run scripts/public-api.py and say so in the pull request."
    )


if __name__ == "__main__":
    main()
