#!/usr/bin/env python3
"""Exposes the crates an example uses to the playground's interpreter (ravenrs-live).

Puts `#[cfg_attr(feature = "live", ravenrs_live::expose)]` on every inherent `impl` block, public
enum, public struct and inline public module of those crates, and writes a `src/live_list.rs`
for each crate that registers them: ravenrs-widgets (with the crates under it), ravenrs-movie,
ravenrs-app and ravenrs-app-ui. Run it after adding a component; scripts/check-component-list.py
fails while it is out of date."""
import pathlib
import re
import subprocess
import sys

root = pathlib.Path(__file__).resolve().parent.parent
check = "--check" in sys.argv

# Each list is written in `crate`, and registers the crates of `sources` in order: `path` is how
# the list names them. `only_reexported` takes just what the crate re-exports, behind `feature`:
# one name, or a name per module with `"*"` for the rest.
# `traits` are traits whose implementations are exposed too, as a type's methods.
TARGETS = [
    {
        "crate": "ravenrs-widgets",
        "views": "crate::live::view",
        "modules": True,
        "sources": [
            {"dir": "ravenrs-core", "path": "ravenrs_core"},
            {"dir": "ravenrs-style", "path": "ravenrs_style"},
            {"dir": "ravenrs-layout", "path": "ravenrs_layout"},
            # `ravenrs-graphics` re-exports all of `ravenrs-barcode` as its `barcode` module: the types
            # are read where they are written, and registered by the path the widgets re-export.
            {"dir": "ravenrs-barcode", "path": "ravenrs_graphics", "only_reexported": True, "feature": "charts"},
            {
                "dir": "ravenrs-graphics",
                "path": "ravenrs_graphics",
                "only_reexported": True,
                "feature": {"flow": "flow", "*": "charts"},
            },
            {"dir": "ravenrs-lottie", "path": "ravenrs_lottie", "only_reexported": True, "feature": "lottie"},
            # The base is its own crate; the widgets re-export every name of it at their root.
            {"dir": "ravenrs-widgets-base", "path": "crate"},
            {"dir": "ravenrs-widgets-kit", "path": "crate"},
            {"dir": "ravenrs-widgets-charts", "path": "crate"},
            {"dir": "ravenrs-widgets-imaging", "path": "crate"},
            {"dir": "ravenrs-widgets-dock", "path": "crate"},
            {"dir": "ravenrs-widgets-scheduler", "path": "crate"},
            {"dir": "ravenrs-widgets-lists", "path": "crate"},
            {"dir": "ravenrs-widgets-grid", "path": "crate"},
            {"dir": "ravenrs-widgets", "path": "crate"},
        ],
    },
    {"crate": "ravenrs-movie", "views": "ravenrs_widgets::__live_view", "sources": [{"dir": "ravenrs-movie", "path": "crate"}]},
    {
        "crate": "ravenrs-app",
        "views": None,
        "traits": ["ObjectSpace"],
        "sources": [{"dir": "ravenrs-app", "path": "crate"}],
    },
    {"crate": "ravenrs-app-ui", "views": "ravenrs_widgets::__live_view", "sources": [{"dir": "ravenrs-app-ui", "path": "crate"}]},
]
# Macros that write an exposed `impl` block for the type they are given, with its part.
BUILDER_MACROS = {"overlay_builders": "builders"}
ATTR = '#[cfg_attr(feature = "live", ravenrs_live::expose{args})]'
IMPL = re.compile(r"^impl(<(?P<generics>[^{]*)>)? (?P<ty>[A-Z]\w*)(<(?P<args>[^{]*)>)? \{\s*$")
TRAIT_IMPL = re.compile(r"^impl (?P<trait>[A-Z]\w*) for (?P<ty>[A-Z]\w*) \{\s*$")
ENUM = re.compile(r"^pub enum (?P<ty>[A-Z]\w*)\s*\{")
STRUCT = re.compile(r"^pub struct (?P<ty>[A-Z]\w*)\s*[({;]")
MODULE = re.compile(r"^pub mod (?P<ty>[a-z]\w*)\s*\{")
VIEW = re.compile(
    r"^impl(<(?P<generics>[^{]*)>)? (View|Component) for (?P<ty>[A-Z]\w*)(<(?P<args>[^{]*)>)?\s*(where|\{)"
)
FROM = re.compile(r"^impl From<(?P<source>[A-Z]\w*)> for (?P<ty>[A-Z]\w*) \{")
DEFAULTS = re.compile(r"^pub struct (?P<ty>[A-Z]\w*)<(?P<params>[^>{]*(<[^>]*>[^>{]*)*)>")


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


def generic_params(generics):
    """The type parameters of `<...>` as (name, bounds), or None with a lifetime or const."""
    params = []
    for param in split_top(generics or ""):
        if param.startswith("'") or param.startswith("const "):
            return None
        name, _, bounds = param.partition(":")
        params.append((name.strip(), bounds))
    return params


def view_type(bounds):
    if "ViewSeq" in bounds:
        return "Vec<crate::AnyView>"
    if "View" in bounds:
        return "crate::AnyView"
    return None


def exports(lib):
    names = set()
    for block in re.findall(r"^pub use [\w:]+::\{([^}]*)\};", lib, re.M | re.S):
        names.update(n.strip().split(" as ")[-1] for n in block.split(","))
    names.update(re.findall(r"^pub use [\w:]+::(\w+);", lib, re.M))
    # What lib.rs defines itself is at the crate's root too.
    names.update(re.findall(r"^pub (?:enum|struct|mod) (\w+)", lib, re.M))
    names.discard("")
    return names


def reexported(src):
    """What a crate re-exports of other crates: {crate: {name: (module path, alias)}}."""
    found = {}
    pattern = re.compile(r"^pub use (ravenrs_\w+)((?:::\w+)*)::(\{[^}]*\}|\w+);", re.M | re.S)
    for path in sorted(src.rglob("*.rs")):
        for crate, module, items in pattern.findall(path.read_text()):
            for item in items.strip("{}").split(","):
                item = item.strip()
                if not item:
                    continue
                name, _, alias = item.partition(" as ")
                found.setdefault(crate, {})[name.strip()] = (module, alias.strip() or None)
    return found


def gated_modules(lib):
    """Modules of lib.rs behind a `cfg`: {module: condition}."""
    return dict((m, c) for c, m in re.findall(r"#\[cfg\((.+?)\)\]\n(?:pub )?mod (\w+);", lib))


def gated_exports(lib):
    """Names lib.rs re-exports behind a `cfg`: {name: condition}."""
    gated = {}
    for condition, items in re.findall(r"#\[cfg\(([^\n]+?)\)\]\npub use [\w:]+?::(\{[^}]*\}|\w+);", lib):
        for item in items.strip("{}").split(","):
            if item.strip():
                gated[item.strip().split(" as ")[-1]] = condition
    return gated


def rust_files(src):
    return [p for p in sorted(src.rglob("*.rs")) if p.name not in ("live_list.rs", "live.rs")]


stale = []
counts = []


def generate(target):
    src = root / "crates" / target["crate"] / "src"
    lib = (src / "lib.rs").read_text()
    exported = exports(lib)
    outside = reexported(src)
    # What the crates re-exported as its own take from others counts too, such as the chart
    # types the widgets' families take from ravenrs-graphics.
    for source in target["sources"]:
        if source["path"] == "crate" and source["dir"] != target["crate"]:
            for crate, found in reexported(root / "crates" / source["dir"] / "src").items():
                outside.setdefault(crate, {}).update(found)
    gates = gated_modules(lib)
    exported_gates = gated_exports(lib)

    # The forms the target's views take, for `impl` blocks of types with free parameters. Its views
    # are written in its own sources and in those it re-exports as its own (`path` "crate").
    own_sources = [src] + [
        root / "crates" / source["dir"] / "src"
        for source in target["sources"]
        if source["path"] == "crate" and source["dir"] != target["crate"]
    ]
    struct_defaults, view_forms, gate_of_view = {}, {}, {}
    for path in (path for own in own_sources for path in rust_files(own)):
        for line in path.read_text().split("\n"):
            if match := DEFAULTS.match(line):
                struct_defaults[match["ty"]] = [
                    (p.partition("=")[0].strip().split(":")[0].strip(), p.partition("=")[2].strip() or None)
                    for p in split_top(match["params"])
                ]
    for own in own_sources:
        for path in rust_files(own):
            module = path.relative_to(own).parts[0].removesuffix(".rs")
            for line in path.read_text().split("\n"):
                match = VIEW.match(line)
                if not match:
                    continue
                params = generic_params(match["generics"])
                if params is None:
                    continue
                known = {name: view_type(bounds) for name, bounds in params}
                args = split_top(match["args"] or "")
                if any(a.startswith("'") for a in args):
                    continue
                defaults = struct_defaults.get(match["ty"], [])
                args += [d for _, d in defaults[len(args):]]
                made = [known.get(a, a) for a in args]
                if any(m is None for m in made):
                    continue
                made = [m if m == "()" or "::" in m or m.startswith("Vec<") or m not in exported else f"crate::{m}" for m in made]
                forms = view_forms.setdefault(match["ty"], [])
                if made not in forms:
                    forms.append(made)
                if module in gates:
                    gate_of_view[match["ty"]] = gates[module]

    def block_types(ty, generics, args, crate_path):
        """How the call names an `impl` block's type, and its `with` option, or None to leave it."""
        params = generic_params(generics)
        if params is None:
            return None
        if not params:
            return "", ""
        args = split_top(args or "")
        fixed = {name: view_type(bounds) for name, bounds in params}
        free = [name for name, kind in fixed.items() if kind is None]
        alternatives = []
        if free:
            if crate_path != "crate":
                return None
            for form in view_forms.get(ty, []):
                alternative = {}
                for name in free:
                    if name not in args or args.index(name) >= len(form):
                        return None
                    alternative[name] = form[args.index(name)]
                if alternative not in alternatives:
                    alternatives.append(alternative)
            if not alternatives:
                return None
        first = alternatives[0] if alternatives else {}
        made = [fixed.get(a) or first.get(a) for a in args]
        if any(m is None for m in made):
            return None
        with_option = "; ".join(", ".join(f"{n} = {t}" for n, t in a.items()) for a in alternatives)
        return "::<" + ", ".join(made) + ">", with_option

    calls, aliases, conversions_from = [], [], []
    for source in target["sources"]:
        crate_src = root / "crates" / source["dir"] / "src"
        crate_path = source["path"]
        names = exports((crate_src / "lib.rs").read_text())
        if source["path"] == "crate":
            # A crate the target re-exports as its own is named by the target's root.
            names = names | exported
        name_gates = gated_exports((crate_src / "lib.rs").read_text())
        wanted = outside.get(crate_path, {}) if source.get("only_reexported") else None
        own = crate_path == "crate"
        seen = {}
        for path in rust_files(crate_src):
            module = path.relative_to(crate_src).parts[0].removesuffix(".rs")
            feature = source.get("feature")
            if isinstance(feature, dict):
                feature = feature.get(module, feature.get("*"))
            gate = f'feature = "{feature}"' if feature else (gates.get(module) if own else None)
            module_cfg = f"    #[cfg({gate})]\n" if gate else ""
            out = []
            for line in path.read_text().split("\n"):
                cfg = module_cfg
                # `impl From<A> for B` lets an `A` go where `impl Into<B>` goes.
                if (found := FROM.match(line)) and wanted is None and found["source"] in names and found["ty"] in names:
                    a, b = f"{crate_path}::{found['source']}", f"{crate_path}::{found['ty']}"
                    conversions_from.append(f'{cfg}    registry.conversion::<{a}, {b}>("{found["source"]}", {b}::from);')
                for macro, part in BUILDER_MACROS.items():
                    if own and (found := re.match(rf"^{macro}!\((?P<ty>[A-Z]\w*),", line)) and found["ty"] in names:
                        calls.append(f"{cfg}    crate::{found['ty']}::__live_expose_{part}(registry);")
                trait_impl = TRAIT_IMPL.match(line)
                if trait_impl and trait_impl["trait"] not in target.get("traits", []):
                    trait_impl = None
                is_module = own and target.get("modules") and MODULE.match(line)
                found = IMPL.match(line) or trait_impl or ENUM.match(line) or STRUCT.match(line) or is_module
                # What a crate shares only with its siblings is hidden, and no one's to call.
                above = [kept.strip() for kept in out[-4:]]
                if found and "#[doc(hidden)]" in above and not IMPL.match(line):
                    found = None
                if not found:
                    out.append(line)
                    continue
                ty = found["ty"]
                if ty in name_gates and not module_cfg:
                    cfg = f"    #[cfg({name_gates[ty]})]\n"
                if wanted is not None and ty not in wanted:
                    out.append(line)
                    continue
                if line.startswith("pub enum"):
                    kind, name = "enum", "variants"
                elif line.startswith("pub struct"):
                    kind, name = "struct", "fields"
                elif is_module:
                    kind, name = "mod", None
                elif trait_impl:
                    kind, name = "trait", trait_impl["trait"].lower()
                else:
                    part = seen.get(ty, 0)
                    seen[ty] = part + 1
                    kind, name = "impl", None if part == 0 else f"part{part}"
                types, with_option = "", ""
                if kind == "impl":
                    made = block_types(ty, found.group("generics"), found.group("args"), crate_path)
                    types, with_option = made if made else (None, "")
                options = [f'part = "{name}"'] if name else []
                if with_option:
                    options.append(f'with = "{with_option}"')
                attr = ATTR.format(args=f"({', '.join(options)})" if options else "")
                previous = out[-1].strip() if out else ""
                if not previous.startswith('#[cfg_attr(feature = "live"'):
                    out.append(attr)
                elif previous != attr:
                    out[-1] = attr
                out.append(line)
                exposed = ty in names or (wanted is not None and ty in wanted)
                if not exposed or types is None:
                    continue
                entry = "__live_expose" + (f"_{name}" if name else "")
                if wanted is not None:
                    module_path, alias = wanted[ty]
                    owner = f"{crate_path}{module_path}::{ty}"
                    if alias:
                        aliases.append(f'{cfg}    registry.alias("{alias}", "{ty}");')
                else:
                    owner = f"{crate_path}::{ty}"
                calls.append(f"{cfg}    {owner}{types}::{entry}(registry);")
            text = "\n".join(out)
            if text != path.read_text():
                stale.append(path.relative_to(root))
                if not check:
                    path.write_text(text)

    conversions = []
    if target["views"]:
        for ty in sorted(view_forms):
            if ty not in exported:
                continue
            gate = exported_gates.get(ty) or gate_of_view.get(ty)
            cfg = f"    #[cfg({gate})]\n" if gate else ""
            for form in view_forms[ty]:
                written = f"crate::{ty}" + (f"<{', '.join(form)}>" if form else "")
                conversions.append(f"{cfg}    {target['views']}::<{written}>(registry);")

    body = "\n".join(calls + aliases + conversions_from + conversions)
    listing = f"""//! Generated by scripts/gen-live.py: what the interpreter of the playground can reach in
//! this crate. Do not edit by hand.

use ravenrs_live::Registry;

/// Registers every exposed type of this crate, and how its views become an `AnyView`.
#[allow(clippy::too_many_lines)]
pub(crate) fn expose_all(registry: &mut Registry) {{
{body}
}}
"""
    # As rustfmt leaves it, so formatting the workspace keeps the list current.
    listing = subprocess.run(
        ["rustfmt", "--edition", "2024", "--config-path", str(root / "rustfmt.toml"), "--emit", "stdout"],
        input=listing, capture_output=True, text=True, check=True, cwd=root,
    ).stdout
    listed = src / "live_list.rs"
    if not listed.exists() or listed.read_text() != listing:
        stale.append(listed.relative_to(root))
        if not check:
            listed.write_text(listing)
    counts.append(f"{target['crate']}: {len(calls)} blocks, {len(conversions)} views")


for target in TARGETS:
    generate(target)
if check and stale:
    sys.exit("out of date, run scripts/gen-live.py: " + ", ".join(map(str, stale)))
print("; ".join(counts))
