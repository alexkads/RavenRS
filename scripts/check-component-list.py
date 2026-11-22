#!/usr/bin/env python3
"""Checks site/public/components/index.html against the code: every component's API link
opens a page of the generated docs, and every public component is listed. Run after
`cargo doc`. scripts/gen-component-pages.py checks that each one has an example."""
import os
import pathlib
import re
import sys

root = pathlib.Path(__file__).resolve().parent.parent
page = (root / "site/public/components/index.html").read_text()
links = {
    name: f"ravenrs_widgets/{path}"
    for path, name in re.findall(r'data-api="((?:\w+/)*struct\.(\w+)\.html)"', page)
}
listed = set(links)

# What the code counts as a public component, shared with scripts/feature-center-coverage.py.
sys.path.insert(0, str(root / "scripts"))
from component_inventory import widget_components  # noqa: E402

views = {name for _, name in widget_components()}

docs = pathlib.Path(os.environ.get("RAVENRS_API_DOCS_DIR", pathlib.Path(os.environ.get("CARGO_TARGET_DIR", root / "target")) / "doc"))
problems = []
for name, path in sorted(links.items()):
    if not (docs / path).exists():
        problems.append(f"{name}: no API page; run cargo doc, or fix the link")
for name in sorted(views - listed):
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
