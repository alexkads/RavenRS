"""What the code says the components are, for the scripts that check them against a list.

- `widget_components()`: every public view of the widgets' crates, as scripts/check-component-list.py
  has always counted them (a `pub struct` with `impl View` or `impl Component`, not `#[doc(hidden)]`).
- `built_components()`: every public type of the widgets' crates that is not a view but is put on
  the screen by its own `build(self, cx: &mut BuildCx)`, like `CablePlug`.
- `app_builders()`: every public type of the application framework (`ravenrs-app*`) configured by
  methods that take `mut self`: `App`, `EntityInfo`, `ApiService`, `SecuredApp` and the rest.
- `public_methods()`: the `pub fn` of a type's own `impl` blocks, what a person can call on it.

It reads the source with regular expressions and a brace counter that skips strings, characters and
comments. That is enough for this codebase, which rustfmt keeps in one shape.
"""
import pathlib
import re

root = pathlib.Path(__file__).resolve().parent.parent

WIDGET_CRATES = (
    "ravenrs-widgets",
    "ravenrs-widgets-base",
    "ravenrs-widgets-kit",
    "ravenrs-widgets-grid",
    "ravenrs-widgets-lists",
    "ravenrs-widgets-scheduler",
    "ravenrs-widgets-dock",
    "ravenrs-widgets-imaging",
    "ravenrs-widgets-charts",
    "ravenrs-widgets-chat",
)

# Parts of a listed component, and plumbing that is not a component on its own.
NOT_LISTED = {
    "DiagnosticsHost", "Command", "EyedropperArea", "ChatMedia", "ListItem", "SidebarGroup",
    "SidebarInset", "SidebarItem", "SidebarSection", "SidebarSkeleton", "SidebarTrigger", "Styled",
    "Faded", "Keyed", "Localized", "Responsive", "FnView", "AccessibleControl",
}

# Crates of the application framework with nothing to show on a screen: the test kit and the
# macros (what the macros make is shown through the types they make).
APP_LEFT_OUT = ("ravenrs-app-testkit", "ravenrs-app-derive")


def _files(crate):
    return sorted((root / "crates" / crate / "src").rglob("*.rs"))


def _hidden(text, start):
    """Whether the item starting at `start` carries #[doc(hidden)] in its attributes."""
    before = text[:start].rstrip("\n").split("\n")
    for line in reversed(before):
        line = line.strip()
        if line.startswith("#[doc(hidden)]"):
            return True
        if not (line.startswith("#[") or line.startswith("///") or line.startswith("//")):
            return False
    return False


def _structs(crate):
    """Each `pub struct` of the crate that is not hidden: name -> (path, line)."""
    found = {}
    for path in _files(crate):
        text = path.read_text()
        for match in re.finditer(r"^pub struct (\w+)", text, re.M):
            if not _hidden(text, match.start()):
                found.setdefault(match.group(1), (path, text.count("\n", 0, match.start()) + 1))
    return found


def _source(crate):
    return "\n".join(path.read_text() for path in _files(crate))


def widget_components():
    """(crate, type) -> (path, line) for every public view of the widgets."""
    components = {}
    for crate in WIDGET_CRATES:
        source = _source(crate)
        for name, place in _structs(crate).items():
            if name in NOT_LISTED:
                continue
            if re.search(rf"impl(<[^>]*>)? (View|Component) for {name}\b", source):
                components[(crate, name)] = place
    return components


def built_components():
    """(crate, type) -> (path, line) for every public type of the widgets that builds itself into a
    `BuildCx` without being a view: `CablePlug::build(self, cx)`."""
    components = {}
    views = widget_components()
    for crate in WIDGET_CRATES:
        for name, place in _structs(crate).items():
            if name in NOT_LISTED or (crate, name) in views:
                continue
            build = public_methods_at(crate, name).get("build")
            if build is None:
                continue
            path, line, _ = build
            signature = " ".join(path.read_text().split("\n")[line - 1 : line + 2])
            if re.search(r"\(\s*&?(mut )?self,\s*cx: &mut BuildCx", signature):
                components[(crate, name)] = place
    return components


def app_crates():
    return sorted(
        path.name
        for path in (root / "crates").glob("ravenrs-app*")
        if (path / "src").is_dir() and path.name not in APP_LEFT_OUT
    )


def app_builders():
    """(crate, type) -> (path, line) for every public type of ravenrs-app* set up by `mut self`."""
    builders = {}
    for crate in app_crates():
        for name, place in _structs(crate).items():
            if any(method_takes_mut_self for method_takes_mut_self in _methods(crate, name).values()):
                builders[(crate, name)] = place
    return builders


def _skip(text, i):
    """If a string, character or comment starts at i, the index after it; else None."""
    if text.startswith("//", i):
        end = text.find("\n", i)
        return len(text) if end < 0 else end
    if text.startswith("/*", i):
        end = text.find("*/", i + 2)
        return len(text) if end < 0 else end + 2
    raw = re.match(r'b?r(#*)"', text[i : i + 12])
    if raw and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_")):
        close = '"' + raw.group(1)
        end = text.find(close, i + raw.end())
        return len(text) if end < 0 else end + len(close)
    if text[i] == '"':
        j = i + 1
        while j < len(text) and text[j] != '"':
            j += 2 if text[j] == "\\" else 1
        return j + 1
    if text[i] == "'":
        char = re.match(r"'(\\[^']{1,10}|[^\\'])'", text[i : i + 13])
        if char:
            return i + char.end()
    return None


def _block_end(text, open_brace):
    """The index just after the `}` that closes the `{` at open_brace."""
    depth, i = 0, open_brace
    while i < len(text):
        skipped = _skip(text, i)
        if skipped is not None:
            i = skipped
            continue
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
            if depth == 0:
                return i + 1
        i += 1
    return len(text)


def _methods(crate, name):
    """method -> whether it takes `mut self`, for the `pub fn` of `name`'s own impl blocks."""
    methods = {}
    for method, (_, _, mut_self) in public_methods_at(crate, name).items():
        methods[method] = mut_self
    return methods


def public_methods_at(crate, name):
    """method -> (path, line, takes `mut self`) for the `pub fn` of `name`'s own impl blocks."""
    head = re.compile(
        rf"^impl(?:<[^{{;]*?>)?\s+(?:[\w:]+::)?{name}(?:<[^{{;]*?>)?\s*(?:where[^{{;]*)?\{{", re.M
    )
    methods = {}
    for path in _files(crate):
        text = path.read_text()
        for match in head.finditer(text):
            start = match.end() - 1
            body = text[start : _block_end(text, start)]
            # Only what sits right inside the block, not in a nested one.
            depth, i, top = 0, 0, []
            while i < len(body):
                skipped = _skip(body, i)
                if skipped is not None:
                    i = skipped
                    continue
                if body[i] == "{":
                    depth += 1
                elif body[i] == "}":
                    depth -= 1
                elif depth == 1:
                    found = re.match(r"pub (?:const )?(?:async )?(?:unsafe )?fn (\w+)\s*(?:<[^(]*>)?\(\s*(mut self)?", body[i:])
                    if found and (i == 0 or body[i - 1] in " \t\n"):
                        if not _hidden(body, i):
                            line = text.count("\n", 0, start + i) + 1
                            methods.setdefault(found.group(1), (path, line, bool(found.group(2))))
                        i += found.end()
                        continue
                i += 1
    return methods


def public_methods(crate, name):
    """The names of the `pub fn` a person can call on `name`."""
    return set(public_methods_at(crate, name))
