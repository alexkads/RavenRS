#!/usr/bin/env python3
"""Generates one page per component under <out>/, from the list in
site/components/index.html and the examples in examples/component-gallery/src/samples.rs.
The code on a page is the example the gallery compiles, so it cannot go stale."""
import html
import pathlib
import re
import sys

root = pathlib.Path(__file__).resolve().parent.parent
out = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else root / "target/site-preview/components"
listing = (root / "site/components/index.html").read_text()
source = (root / "examples/component-gallery/src/samples.rs").read_text()

ROW = re.compile(
    r'<li><a href="(\w+)/" data-api="([^"]+)">(\w+)</a><span class="sep"> - </span><span class="desc">(.*?)</span></li>'
)
SAMPLE = re.compile(r"^// sample: (\w+), (\d+)\n(.*?)^// end$", re.M | re.S)
KEYWORDS = "as|break|const|continue|else|enum|fn|for|if|impl|in|let|loop|match|mod|move|mut|pub|return|struct|trait|use|where|while"
TOKEN = re.compile(
    r'(?P<string>"(?:[^"\\]|\\.)*")|(?P<comment>//[^\n]*)|(?P<keyword>\b(?:' + KEYWORDS + r')\b)'
    r"|(?P<type>\b[A-Z]\w*\b)|(?P<number>\b\d[\d_.]*\b)"
)


def highlight(code):
    parts, at = [], 0
    for match in TOKEN.finditer(code):
        parts.append(html.escape(code[at : match.start()], quote=False))
        parts.append(f'<span class="t-{match.lastgroup}">{html.escape(match.group(), quote=False)}</span>')
        at = match.end()
    parts.append(html.escape(code[at:], quote=False))
    return "".join(parts)


samples = {name: (int(height), code.rstrip() + "\n") for name, height, code in SAMPLE.findall(source)}
# The framework's window is shown in the gallery, but is not a component of ravenrs-widgets.
samples.pop("AppShell", None)
rows = [(slug, api, name, description) for slug, api, name, description in ROW.findall(listing)]
ordered = sorted(rows, key=lambda row: row[2].lower())

missing = [row[2] for row in rows if row[2] not in samples]
# These live in ravenrs-movie, and this list is of ravenrs-widgets.
elsewhere = {"MovieAssistant", "MoviePlayer"}
extra = sorted(set(samples) - {row[2] for row in rows} - elsewhere)
if missing or extra or not rows:
    sys.exit(f"the list and the examples disagree: without an example {missing}, not listed {extra}")

TEMPLATE = """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>{name} · RavenRS</title>
<meta name="description" content="{plain}">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500&display=swap">
<link rel="stylesheet" href="../style.css">
</head>
<body>
<nav class="nav" aria-label="Main">
  <a class="brand" href="../../"><span class="mark"><i></i></span>RavenRS</a>
  <ul>
    <li><a href="../" aria-current="page">Components</a></li>
    <li class="hide-sm"><a href="../../demo/">Live demo</a></li>
    <li class="hide-sm"><a href="../../playground/">Playground</a></li>
    <li class="hide-sm"><a href="../../designer/">Reports</a></li>
    <li><a href="../../api/ravenrs/">API</a></li>
    <li class="hide-sm"><a href="https://github.com/alexkads/RavenRS">GitHub</a></li>
  </ul>
</nav>
<div class="layout">
  <aside class="side" aria-label="All components">
    <h2>A to Z</h2>
    <ul>{index}</ul>
  </aside>
  <script src="../side.js"></script>
  <main>
    <p class="crumb"><a href="../">Components</a> / {name}</p>
    <h1>{name}</h1>
    <p class="lede">{description}.</p>
    <h2 class="block">Example</h2>
    <p class="sub">The component itself, running in WebAssembly, beside its code in a playground. Try it.</p>
    <iframe class="stage" src="../../demo/?component={name}" title="Live example of {name}" height="{height}"></iframe>
    <div class="block-head">
      <h2 class="block">Code</h2>
      <button class="copy" type="button" hidden>Copy</button>
    </div>
    <pre class="code" tabindex="0"><code>{code}</code></pre>
    <p><a class="more" href="../../playground/?example={name}">Open in the playground, with every example →</a></p>
    <p><a class="more" href="../../api/ravenrs_widgets/{api}">Full API of {name} →</a></p>
    <nav class="pager" aria-label="Other components">{previous}{following}</nav>
  </main>
</div>
<script>
  const copy = document.querySelector(".copy");
  if (navigator.clipboard) {{
    copy.hidden = false;
    copy.addEventListener("click", async () => {{
      await navigator.clipboard.writeText(document.querySelector(".code").textContent);
      copy.textContent = "Copied";
      setTimeout(() => (copy.textContent = "Copy"), 1500);
    }});
  }}
</script>
</body>
</html>
"""

# What the playground of one example takes, in logical pixels: the page's padding and the bar,
# then the taller of the code (its lines and the status under it) and the example (its own
# height, the caption and padding around it, and the folded output). The gallery's tests check
# these against its layout.
PAGE, BAR, LINE, EDITOR, STATUS, AROUND_EXAMPLE = 32, 89, 22, 12, 46, 127


def with_uses(code):
    """The code as the playground shows it: what the gallery's file brings in for every example,
    said in the ones that use it (`sample_code` in examples/component-gallery does the same)."""
    uses = ""
    for module, names in (("std::collections", ["BTreeSet"]), ("ravenrs::layout", ["Axis", "LayoutStyle"])):
        used = [name for name in names if re.search(r"(?<![\w:])" + name + r"(?!\w)", code)]
        if len(used) == 1:
            uses += f"use {module}::{used[0]};\n"
        elif used:
            uses += f"use {module}::{{{', '.join(used)}}};\n"
    return uses + "use ravenrs::prelude::*;\n\n" + code


def stage_height(code, example):
    # Its lines, and the empty one after.
    lines = code.count("\n") + 1
    return PAGE + BAR + max(lines * LINE + EDITOR + STATUS, example + AROUND_EXAMPLE)


for position, (slug, api, name, description) in enumerate(ordered):
    height, code = samples[name]
    code = with_uses(code)
    current = ' aria-current="page"'
    index = "".join(
        f'<li><a href="../{other[0]}/"{current if other[2] == name else ""}>{other[2]}</a></li>' for other in ordered
    )
    previous = (
        f'<a href="../{ordered[position - 1][0]}/">← {ordered[position - 1][2]}</a>' if position > 0 else "<span></span>"
    )
    following = (
        f'<a href="../{ordered[position + 1][0]}/">{ordered[position + 1][2]} →</a>' if position + 1 < len(ordered) else ""
    )
    page = TEMPLATE.format(
        name=name,
        plain=html.escape(html.unescape(description)),
        description=description,
        index=index,
        height=stage_height(code, height),
        code=highlight(code),
        api=api,
        previous=previous,
        following=following,
    )
    folder = out / slug
    folder.mkdir(parents=True, exist_ok=True)
    (folder / "index.html").write_text(page)

print(f"{len(rows)} component pages in {out}")
