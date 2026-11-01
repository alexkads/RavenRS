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
rows = [(slug, api, name, description) for slug, api, name, description in ROW.findall(listing)]
ordered = sorted(rows, key=lambda row: row[2].lower())

missing = [row[2] for row in rows if row[2] not in samples]
extra = sorted(set(samples) - {row[2] for row in rows})
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
  <main>
    <p class="crumb"><a href="../">Components</a> / {name}</p>
    <h1>{name}</h1>
    <p class="lede">{description}.</p>
    <h2 class="block">Example</h2>
    <p class="sub">This is the component itself, running in WebAssembly. Try it.</p>
    <iframe class="stage" src="../../demo/?component={name}" title="Live example of {name}" height="{height}"></iframe>
    <div class="block-head">
      <h2 class="block">Code</h2>
      <button class="copy" type="button" hidden>Copy</button>
    </div>
    <pre class="code" tabindex="0"><code>{code}</code></pre>
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

for position, (slug, api, name, description) in enumerate(ordered):
    height, code = samples[name]
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
        height=height + 48,
        code=highlight("use ravenrs::prelude::*;\n\n" + code),
        api=api,
        previous=previous,
        following=following,
    )
    folder = out / slug
    folder.mkdir(parents=True, exist_ok=True)
    (folder / "index.html").write_text(page)

print(f"{len(rows)} component pages in {out}")
