#!/usr/bin/env python3
"""Checks site/components/index.html against the code: every component's API link
opens a page of the generated docs, and every public component is listed. Run after
`cargo doc`. scripts/gen-component-pages.py checks that each one has an example."""
import os
import pathlib
import re
import sys

root = pathlib.Path(__file__).resolve().parent.parent
page = (root / "site/components/index.html").read_text()
links = {
    name: f"ravenrs_widgets/{path}"
    for path, name in re.findall(r'data-api="((?:\w+/)*struct\.(\w+)\.html)"', page)
}
listed = set(links)

# Parts of a listed component, and plumbing that is not a component on its own.
NOT_LISTED = {"DiagnosticsHost", "Command", "EyedropperArea", "ChatMedia", "ListItem", "SidebarGroup", "SidebarInset", "SidebarItem", "SidebarSection", "SidebarSkeleton", "SidebarTrigger", "Styled", "Faded", "Keyed", "Localized", "Responsive", "FnView", "AccessibleControl"}

# The widgets and the base they re-export; what the base shares only with them is hidden.
source = "\n".join(
    p.read_text() for crate in ("ravenrs-widgets", "ravenrs-widgets-base") for p in (root / "crates" / crate / "src").rglob("*.rs")
)
public = set(re.findall(r"^pub struct (\w+)", source, re.M)) - set(
    re.findall(r"^#\[doc\(hidden\)\]\n(?:#\[.*\]\n|//.*\n)*pub struct (\w+)", source, re.M)
)
views = {name for name in public if re.search(rf"impl(<[^>]*>)? (View|Component) for {name}\b", source)}

docs = pathlib.Path(os.environ.get("RAVENRS_API_DOCS_DIR", pathlib.Path(os.environ.get("CARGO_TARGET_DIR", root / "target")) / "doc"))
problems = []
for name, path in sorted(links.items()):
    if not (docs / path).exists():
        problems.append(f"{name}: no API page; run cargo doc, or fix the link")
for name in sorted(views - listed - NOT_LISTED):
    problems.append(f"{name}: public component missing from the list")

# The playground's interpreter has to know every component as it is.
import subprocess
generated = subprocess.run([sys.executable, str(root / "scripts/gen-live.py"), "--check"], capture_output=True, text=True)
if generated.returncode != 0:
    problems.append(generated.stderr.strip() or generated.stdout.strip())

if problems:
    print("\n".join(problems), file=sys.stderr)
    sys.exit(1)
print(f"{len(listed)} components listed, all with API pages")
