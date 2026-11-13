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
import re
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


def rustdoc_json(package: str, hidden: bool = False) -> dict:
    """rustdoc's JSON of a package. `hidden` keeps the items of hidden modules, which a crate the
    package re-exports from has: its public names live there."""
    target = Path(os.environ.get("CARGO_TARGET_DIR") or ROOT / "target")
    command = [
        "cargo", "+nightly", "rustdoc", "-q", "-p", package, "--all-features",
        "--", "-Z", "unstable-options", "--output-format", "json",
    ]
    if hidden:
        command.append("--document-hidden-items")
    done = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
    if done.returncode != 0:
        sys.exit(f"rustdoc failed (is the nightly toolchain installed?):\n{done.stderr[-4000:]}")
    return json.loads((target / "doc" / f"{package.replace('-', '_')}.json").read_text())


class Doc:
    """One crate's JSON: its items by id, and where each id lives."""

    def __init__(self, data: dict):
        self.data, self.index, self.paths = data, data["index"], data["paths"]
        self.name = data["index"][str(data["root"])]["name"]
        self.by_path = {
            tuple(entry["path"]): id_ for id_, entry in self.paths.items() if entry["crate_id"] == 0
        }

    def item(self, id_):
        """The item, unless it is hidden from the documentation, as rustdoc leaves it out."""
        it = self.index.get(str(id_))
        if it and any("doc(hidden)" in str(attr) for attr in it.get("attrs", [])):
            return None
        return it

    def canonical(self, id_):
        """The crate and path where an id is defined, as rustdoc names it."""
        entry = self.paths.get(str(id_))
        return tuple(entry["path"]) if entry else None


def api(docs: dict[str, Doc], root: str) -> list[str]:
    """Walks `root` from its top, opening items re-exported from the other crates in `docs`."""
    lines: set[str] = set()
    seen: set[tuple[str, str, str]] = set()
    public: dict[tuple, set[str]] = {}

    def members(doc: Doc, path: str, it: dict):
        kind, inner = next(iter(it["inner"].items()))
        if kind == "struct" and isinstance(inner["kind"], dict):
            fields = list(inner["kind"].get("plain", {}).get("fields", []))
            fields += inner["kind"].get("tuple", []) or []
            for field_id in fields:
                field = doc.item(field_id) if field_id is not None else None
                if field and field["visibility"] == "public" and field["name"]:
                    lines.add(f"field {path}::{field['name']}")
        if kind == "enum":
            for variant_id in inner["variants"]:
                variant = doc.item(variant_id)
                if variant:
                    lines.add(f"variant {path}::{variant['name']}")
        if kind == "trait":
            for member_id in inner["items"]:
                member = doc.item(member_id)
                if member and member["name"]:
                    lines.add(f"trait-item {path}::{member['name']}")
        for impl_id in inner.get("impls", []):
            impl = doc.item(impl_id)
            if not impl:
                continue
            body = impl["inner"]["impl"]
            if body["is_synthetic"] or body["blanket_impl"] is not None or body["trait"] is not None:
                continue
            for member_id in body["items"]:
                member = doc.item(member_id)
                if member and member["visibility"] == "public" and member["name"]:
                    member_kind = KINDS.get(next(iter(member["inner"])), "item")
                    lines.add(f"{'method' if member_kind == 'fn' else member_kind} {path}::{member['name']}")

    def visit(doc: Doc, id_, path: str):
        it = doc.item(id_)
        if it is None and str(id_) in doc.index:
            return
        if it is None:
            source = doc.canonical(id_)
            # From a crate of the same family: listed as if it were here, by its path here.
            inner_doc = docs.get(source[0]) if source else None
            if inner_doc and source in inner_doc.by_path:
                visit(inner_doc, inner_doc.by_path[source], path)
            else:
                lines.add(f"reexport {path} = {'::'.join(source) if source else '?'}")
            return
        key = (doc.name, path, str(id_))
        if key in seen:
            return
        seen.add(key)
        kind, inner = next(iter(it["inner"].items()))
        if kind == "module":
            if path:
                lines.add(f"mod {path}")
            for child_id in inner["items"]:
                child = doc.item(child_id)
                if child is None or child["visibility"] != "public":
                    continue
                child_kind, child_inner = next(iter(child["inner"].items()))
                if child_kind == "use":
                    target = child_inner["id"]
                    if child_inner["is_glob"]:
                        glob = doc.item(target) if target is not None else None
                        if glob and "module" in glob["inner"]:
                            for grand_id in glob["inner"]["module"]["items"]:
                                grand = doc.item(grand_id)
                                if grand and grand["visibility"] == "public" and grand["name"]:
                                    visit(doc, grand_id, join(path, grand["name"]))
                        else:
                            lines.add(f"reexport {join(path, '*')} = {child_inner['source']}")
                        continue
                    if target is None:
                        lines.add(f"reexport {join(path, child_inner['name'])} = {child_inner['source']}")
                        continue
                    visit(doc, target, join(path, child_inner["name"]))
                elif child["name"] and child_kind != "impl":
                    visit(doc, child_id, join(path, child["name"]))
            return
        if kind == "use":
            return
        lines.add(f"{KINDS.get(kind, kind)} {path}")
        canonical = doc.canonical(id_)
        if canonical:
            public.setdefault(canonical, set()).add(path)
        members(doc, path, it)

    def join(prefix: str, name: str) -> str:
        return f"{prefix}::{name}" if prefix else name

    top = docs[root]
    visit(top, top.data["root"], "")
    # Traits a public type implements, wherever the impl is written.
    for doc in docs.values():
        for id_, it in doc.index.items():
            body = it["inner"].get("impl")
            if body and doc.item(id_) is None:
                continue
            if not body or body["trait"] is None or body["is_synthetic"] or body["blanket_impl"] is not None:
                continue
            target = body["for"].get("resolved_path") if isinstance(body["for"], dict) else None
            if not target:
                continue
            trait = body["trait"]["path"].split("::")[-1]
            # The compiler's own markers for derived traits are no one's API.
            if trait.startswith("Structural") or trait == "TrivialClone":
                continue
            for path in public.get(doc.canonical(target["id"]), ()):
                lines.add(f"impl {trait} for {path}")
    return sorted(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("-p", "--package", default="ravenrs-widgets")
    parser.add_argument("--check", action="store_true", help="fail if the list changed")
    parser.add_argument(
        "--with",
        action="append",
        default=None,
        help="a crate the package re-exports and whose items count as its own; by default, its dependencies named after it",
    )
    args = parser.parse_args()
    file = ROOT / "crates" / args.package / "public-api.txt"
    header = [
        f"# The public API of {args.package}, by the paths its users write.",
        "# Written by scripts/public-api.py; `--check` fails when it changes. Regenerate it only",
        "# when the change is meant, and say so in the pull request.",
    ]
    inner = args.__dict__["with"]
    if inner is None:
        # The crates it is split into: its dependencies named after it, such as ravenrs-widgets-base.
        manifest = (ROOT / "crates" / args.package / "Cargo.toml").read_text()
        dependencies = manifest.split("[dependencies]", 1)[-1].split("\n[", 1)[0]
        inner = re.findall(rf"^({re.escape(args.package)}-[\w-]+)", dependencies, re.M)
    docs = {}
    for package in [args.package, *inner]:
        doc = Doc(rustdoc_json(package, hidden=package != args.package))
        docs[doc.name] = doc
    now = header + api(docs, args.package.replace("-", "_"))
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
