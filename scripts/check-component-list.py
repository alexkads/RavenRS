#!/usr/bin/env python3
"""Checks site/components/index.html against the code: every component's API link
opens a page of the generated docs, and every public component is listed. Run after
`cargo doc`. scripts/gen-component-pages.py checks that each one has an example."""
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
NOT_LISTED = {"Command", "ListItem", "SidebarGroup", "SidebarInset", "SidebarItem", "SidebarSection", "SidebarSkeleton", "SidebarTrigger", "Styled", "Keyed", "Localized", "Responsive", "FnView"}

source = "\n".join(p.read_text() for p in (root / "crates/ravenrs-widgets/src").rglob("*.rs"))
public = set(re.findall(r"^pub struct (\w+)", source, re.M))
views = {name for name in public if re.search(rf"impl(<[^>]*>)? (View|Component) for {name}\b", source)}

problems = []
for name, path in sorted(links.items()):
    if not (root / "target/doc" / path).exists():
        problems.append(f"{name}: no API page; run cargo doc, or fix the link")
for name in sorted(views - listed - NOT_LISTED):
    problems.append(f"{name}: public component missing from the list")

if problems:
    print("\n".join(problems), file=sys.stderr)
    sys.exit(1)
print(f"{len(listed)} components listed, all with API pages")
