#!/usr/bin/env python3
"""Exposes ravenrs-widgets to the playground's interpreter (ravenrs-live).

Puts `#[cfg_attr(feature = "live", ravenrs_live::expose)]` on every inherent `impl` block and
public enum of the crate, and writes `src/live_list.rs`, which calls what each one generated and
says which types are views. Run it after adding a component; scripts/check-component-list.py
fails while it is out of date."""
import pathlib
import re
import sys

root = pathlib.Path(__file__).resolve().parent.parent
src = root / "crates/ravenrs-widgets/src"
# The crates whose types an example uses, in the order they register; views are in the last.
CRATES = [("ravenrs-core", "ravenrs_core"), ("ravenrs-style", "ravenrs_style"), ("ravenrs-layout", "ravenrs_layout"), ("ravenrs-widgets", "crate")]
ATTR = '#[cfg_attr(feature = "live", ravenrs_live::expose{args})]'
IMPL = re.compile(r"^impl(<(?P<generics>[^{]*)>)? (?P<ty>[A-Z]\w*)(<(?P<args>[^{]*)>)? \{\s*$")
ENUM = re.compile(r"^pub enum (?P<ty>[A-Z]\w*)\s*\{")
VIEW = re.compile(r"^impl(<[^{]*>)? (View|Component) for (?P<ty>[A-Z]\w*)(<[^{]*>)?\s*(where|\{)")
check = "--check" in sys.argv


def split_top(text):
    parts, depth, current = [], 0, ""
    for c in text:
        if c in "<([":
            depth += 1
        elif c in ">)]":
            depth -= 1
        if c == "," and depth == 0:
            parts.append(current.strip())
            current = ""
        else:
            current += c
    if current.strip():
        parts.append(current.strip())
    return parts


def concrete(generics, args):
    """The type arguments as concrete types, or None when a parameter cannot be made concrete."""
    if not generics:
        return ""
    params = {}
    for param in split_top(generics):
        if param.startswith("'") or param.startswith("const "):
            return None
        name, _, bounds = param.partition(":")
        if "ViewSeq" in bounds:
            params[name.strip()] = "Vec<crate::AnyView>"
        elif "View" in bounds:
            params[name.strip()] = "crate::AnyView"
        else:
            return None
    made = [params.get(a.strip()) for a in split_top(args or "")]
    if any(m is None for m in made):
        return None
    return "::<" + ", ".join(made) + ">"


def exports(lib):
    names = set()
    for block in re.findall(r"^pub use [\w:]+::\{([^}]*)\};", lib, re.M | re.S):
        names.update(n.strip().split(" as ")[-1] for n in block.split(","))
    names.update(re.findall(r"^pub use [\w:]+::(\w+);", lib, re.M))
    # What lib.rs defines itself is at the crate's root too.
    names.update(re.findall(r"^pub (?:enum|struct) (\w+)", lib, re.M))
    names.discard("")
    return names


calls, views, stale, concrete_of = [], {}, [], {}
exported = set()
for crate_dir, crate_path in CRATES:
    crate_src = root / "crates" / crate_dir / "src"
    names = exports((crate_src / "lib.rs").read_text())
    if crate_path == "crate":
        exported = names
    seen = {}
    for path in sorted(crate_src.rglob("*.rs")):
        if path.name in ("live_list.rs", "live.rs"):
            continue
        lines = path.read_text().split("\n")
        out = []
        for line in lines:
            if crate_path == "crate" and (match := VIEW.match(line)):
                views[match["ty"]] = "<" in line.split(" for ")[1].split("{")[0].split(" where")[0]
            target = IMPL.match(line) or ENUM.match(line)
            if target:
                ty = target["ty"]
                is_enum = line.startswith("pub enum")
                part = seen.get(ty, 0)
                seen[ty] = part + 1
                name = "variants" if is_enum else (None if part == 0 else f"part{part}")
                args = f'(part = "{name}")' if name else ""
                attr = ATTR.format(args=args)
                previous = out[-1].strip() if out else ""
                if not previous.startswith('#[cfg_attr(feature = "live"'):
                    out.append(attr)
                elif previous != attr:
                    out[-1] = attr
                if ty in names:
                    types = "" if is_enum else concrete(target.group("generics"), target.group("args"))
                    if types is not None:
                        if crate_path == "crate":
                            concrete_of.setdefault(ty, types)
                        entry = "__live_expose" + (f"_{name}" if name else "")
                        calls.append(f"    {crate_path}::{ty}{types}::{entry}(registry);")
            out.append(line)
        text = "\n".join(out)
        if text != path.read_text():
            stale.append(path.relative_to(root))
            if not check:
                path.write_text(text)

conversions = []
for ty, generic in sorted(views.items()):
    if ty not in exported:
        continue
    if generic:
        if not concrete_of.get(ty):
            continue
        conversions.append(f"crate::{ty}<{concrete_of[ty][3:-1]}>")
    else:
        conversions.append(f"crate::{ty}")

body = "\n".join(calls + [f"    crate::live::view::<{c}>(registry);" for c in conversions])
listing = f"""//! Generated by scripts/gen-live.py: what the interpreter of the playground can reach in
//! this crate. Do not edit by hand.

use ravenrs_live::Registry;

/// Registers every exposed type of this crate, and how its views become an `AnyView`.
#[allow(clippy::too_many_lines)]
pub(crate) fn expose_all(registry: &mut Registry) {{
{body}
}}
"""
target = src / "live_list.rs"
if not target.exists() or target.read_text() != listing:
    stale.append(target.relative_to(root))
    if not check:
        target.write_text(listing)
if check and stale:
    sys.exit("out of date, run scripts/gen-live.py: " + ", ".join(map(str, stale)))
print(f"{len(calls)} blocks exposed, {len(conversions)} views")
