// The docs portal is a static rendering of the repository's Markdown, built locally.
import { readFile, writeFile, mkdir, copyFile, cp } from "node:fs/promises";
import { createRequire } from "node:module";
import { fileURLToPath, pathToFileURL } from "node:url";
import { resolve, dirname, relative, posix } from "node:path";
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const require = createRequire(resolve(root, "site/docs/package.json"));
const { marked, Renderer } = await import(
  pathToFileURL(require.resolve("marked")).href
);
const hljs = require("highlight.js");
const out = resolve(
  process.argv[2] || resolve(root, "target/docs-preview/docs"),
);
const version = (await readFile(resolve(root, "Cargo.toml"), "utf8")).match(
  /\[workspace\.package\][\s\S]*?version = "([^"]+)"/,
)[1];
const siteUrl = (process.env.RAVENRS_SITE_URL || "https://alexkads.github.io/RavenRS").replace(/\/$/, "");
const documents = [];
const pages = [
  ["Start here", "", "Introduction", "docs/README.md"],
  ["Start here", "getting-started", "Installation", "docs/GETTING_STARTED.md"],
  [
    "Start here",
    "cli",
    "CLI & AI assistants",
    "crates/cargo-ravenrs/README.md",
  ],
  ["Start here", "ai", "AI-assisted development", "docs/AI_DEVELOPMENT.md"],
  ["Build applications", "architecture", "Architecture", "ARCHITECTURE.md"],
  [
    "Build applications",
    "design-system",
    "Design system",
    "docs/design-system/README.md",
  ],
  [
    "Build applications",
    "applications",
    "Records & forms",
    "docs/APP-FRAMEWORK.md",
  ],
  ["Build applications", "components", "Component coverage", "docs/PARITY.md"],
  ["Reports", "reporting", "Create reports", "docs/REPORTING.md"],
  ["Reports", "report-server", "Report server", "docs/REPORT-SERVER.md"],
  ["Reference", "quality", "Quality & accessibility", "docs/QUALITY.md"],
  ["Reference", "performance", "Performance", "docs/PERFORMANCE.md"],
  ["Reference", "versioning", "Versions & upgrades", "docs/VERSIONING.md"],
];
const sourceRoutes = new Map(pages.map(([, slug, , source]) => [source, slug]));
const escape = (s) =>
  String(s)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
const slugify = (s) =>
  s
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase()
    .replace(/<[^>]+>/g, "")
    .replace(/&[^;]+;/g, "")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
const arrow = '<span aria-hidden="true">↗</span>';
const searchIndex = [];
await mkdir(out, { recursive: true });
for (const asset of ["docs.css", "docs.js"])
  await copyFile(resolve(root, "site/docs", asset), resolve(out, asset));
await cp(resolve(root, "docs/media"), resolve(out, "_media"), {
  recursive: true,
});
for (const [index, [group, slug, title, source]] of pages.entries()) {
  const dir = resolve(out, slug);
  await mkdir(dir, { recursive: true });
  const base = slug ? "../" : "./";
  const site = `${base}../`;
  const route = (pageSlug) => `${base}${pageSlug ? `${pageSlug}/` : ""}`;
  const raw = (await readFile(resolve(root, source), "utf8")).replace(
    /^---\n[\s\S]*?\n---\n/,
    "",
  );
  documents.push({ title, slug, source, raw });
  await writeFile(resolve(dir, "index.md"), raw);
  const headings = [];
  const used = new Map();
  const renderer = new Renderer();
  renderer.code = ({ text, lang }) => {
    const language = (lang || "text").split(/\s/)[0];
    const aliases = { sh: "bash", shell: "bash", toml: "ini", rs: "rust" };
    const highlight = aliases[language] || language;
    const html = hljs.getLanguage(highlight)
      ? hljs.highlight(text, { language: highlight, ignoreIllegals: true })
          .value
      : escape(text);
    return `<div class="codeblock"><div class="codebar"><span class="code-language">${escape(language)}</span><button type="button" data-copy-code aria-label="Copy ${escape(language)} code">Copy <span aria-hidden="true">⧉</span></button></div><pre tabindex="0"><code>${html}</code></pre></div>\n`;
  };
  let content = marked.parse(raw, { renderer, gfm: true });
  content = content.replace(
    /<h([1-6])>([\s\S]*?)<\/h\1>/g,
    (_, level, body) => {
      const plain = body.replace(/<[^>]+>/g, "").replaceAll("&amp;", "&");
      const initial = slugify(plain);
      const count = used.get(initial) || 0;
      used.set(initial, count + 1);
      const id = `${initial}${count ? `-${count}` : ""}`;
      if (Number(level) === 2 || Number(level) === 3)
        headings.push({ id, title: plain, level: Number(level) });
      return `<h${level} id="${id}">${body}${level !== "1" ? `<a class="heading-anchor" href="#${id}" aria-label="Link to ${escape(plain)}">#</a>` : ""}</h${level}>`;
    },
  );
  content = content.replace(/(href|src)="([^"]+)"/g, (all, attribute, url) => {
    if (/^(?:[a-z]+:|\/|#)/i.test(url)) return all;
    const [path, anchor] = url.split("#");
    const target = posix.normalize(posix.join(posix.dirname(source), path));
    if (sourceRoutes.has(target))
      return `${attribute}="${route(sourceRoutes.get(target))}${anchor ? `#${anchor}` : ""}"`;
    if (target.startsWith("docs/media/"))
      return `${attribute}="${base}_media/${target.slice("docs/media/".length)}"`;
    return `${attribute}="https://github.com/alexkads/RavenRS/blob/master/${target}${anchor ? `#${anchor}` : ""}"`;
  });
  const groups = [...new Set(pages.map((p) => p[0]))];
  const nav = groups
    .map(
      (section) =>
        `<section class="nav-group"><h2>${section}</h2>${pages
          .filter((p) => p[0] === section)
          .map(
            ([, s, t]) =>
              `<a href="${route(s)}" ${s === slug ? 'aria-current="page"' : ""}>${t}</a>`,
          )
          .join("")}</section>`,
    )
    .join("");
  const pager = (offset) => {
    const page = pages[index + offset];
    return page
      ? `<a class="pager-card ${offset > 0 ? "next" : ""}" href="${route(page[1])}"><span>${offset > 0 ? "Next" : "Previous"}</span><strong>${offset > 0 ? "" : "← "}${page[2]}${offset > 0 ? " →" : ""}</strong></a>`
      : "<span></span>";
  };
  const toc = headings
    .map(
      (h) =>
        `<a class="toc-level-${h.level}" href="#${h.id}">${escape(h.title)}</a>`,
    )
    .join("");
  const canonical = `${siteUrl}/docs/${slug ? `${slug}/` : ""}`;
  const structured = JSON.stringify({ "@context": "https://schema.org", "@type": "TechArticle", headline: title, url: canonical, version, about: { "@type": "SoftwareSourceCode", name: "RavenRS", codeRepository: "https://github.com/alexkads/RavenRS", programmingLanguage: "Rust", description: "Declarative, reactive Rust GUI framework for native desktop and WebAssembly applications, with GPU rendering, design tokens, forms and reporting." } }).replaceAll("<", "\\u003c");
  const html = `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover">
<title>${escape(title)} · RavenRS Docs</title><meta name="description" content="${escape(title)} — the RavenRS guide to native and web applications in Rust.">
<link rel="canonical" href="${escape(canonical)}"><meta property="og:title" content="${escape(title)} · RavenRS Docs"><meta property="og:type" content="article"><meta property="og:url" content="${escape(canonical)}"><script type="application/ld+json">${structured}</script>
<meta name="color-scheme" content="light dark"><link rel="stylesheet" href="${base}docs.css">
<script>try{const t=localStorage.getItem('ravenrs-docs-theme');if(t)document.documentElement.dataset.theme=t}catch{}</script>
<script type="module" src="${base}docs.js"></script></head>
<body data-docs-base="${base}"><a class="skip" href="#content">Skip to content</a>
<header class="header"><div class="header-brand"><a href="${site}" class="brand" aria-label="RavenRS home"><span class="raven-mark" aria-hidden="true">R</span>RavenRS</a><span class="brand-divider">/</span><a href="${base}" class="docs-brand">Docs</a></div>
<nav class="topnav" aria-label="Main"><a href="${base}" aria-current="page">Documentation</a><a href="${site}components/">Components</a><a href="${site}demo/">Examples</a></nav>
<div class="header-actions"><button class="search-trigger" data-open-search><span aria-hidden="true">⌕</span><span class="search-label">Search documentation...</span><kbd>⌘ K</kbd></button><a class="github-link" href="https://github.com/alexkads/RavenRS">GitHub ${arrow}</a><button class="icon-button" id="theme-toggle" aria-label="Toggle color theme" title="Toggle color theme">◐</button><button class="icon-button mobile-menu" id="open-nav" aria-label="Open navigation">☰</button></div></header>
<div class="docs-layout"><aside class="sidebar" aria-label="Documentation"><div class="version"><span>RavenRS</span><span class="version-label">v${version}<i></i></span></div>${nav}<div class="sidebar-bottom"><a href="${site}api/ravenrs/">API reference ${arrow}</a><a href="https://github.com/alexkads/RavenRS/discussions">Community ${arrow}</a></div></aside>
<main id="content" tabindex="-1"><div class="article-top"><div class="breadcrumb"><a href="${base}">Docs</a><span>/</span><span>${escape(group)}</span><span>/</span><strong>${escape(title)}</strong></div><button class="page-copy" id="copy-page">Copy page <span aria-hidden="true">⧉</span></button></div><article class="prose">${content}</article><div class="page-feedback"><span>Help us improve these docs.</span><a href="https://github.com/alexkads/RavenRS/edit/master/${source}">Edit this page on GitHub ${arrow}</a><a href="index.md">View Markdown ${arrow}</a></div><nav class="pager" aria-label="Previous and next pages">${pager(-1)}${pager(1)}</nav><footer class="footer">RavenRS ${version}<span>One UI. Native and web.</span></footer></main>
<aside class="toc" aria-label="On this page"><h2>On this page</h2><nav>${toc}</nav><div class="toc-help"><span>Ready to build?</span><a href="${route("getting-started")}">Create your first app →</a></div></aside></div>
<dialog id="search-dialog" aria-label="Search documentation"><div class="search-field"><span aria-hidden="true">⌕</span><input id="docs-search" type="search" placeholder="What are you looking for?" aria-label="Search documentation" autocomplete="off"><button class="icon-button" data-close-search aria-label="Close search">Esc</button></div><div id="search-results" role="status"></div><div class="search-footer"><span>↑ ↓ to navigate</span><span>↵ to open</span><span>esc to close</span></div></dialog>
<dialog id="mobile-nav" aria-label="Documentation navigation"><button class="icon-button close-nav" aria-label="Close navigation">✕</button><div class="version"><span>RavenRS Docs</span><span>v${version}</span></div>${nav}</dialog><div id="copy-status" class="sr-only" role="status"></div></body></html>`;
  await writeFile(resolve(dir, "index.html"), html);
  const plain = raw
    .replace(/```[^\n]*\n([\s\S]*?)```/g, "$1")
    .replace(/!\[[^\]]*\]\([^)]*\)/g, "")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/[#*`>|]/g, "")
    .replace(/\s+/g, " ");
  searchIndex.push({
    title,
    group,
    path: slug ? `${slug}/` : "./",
    text: plain,
    headings,
  });
}
await writeFile(resolve(out, "search.json"), JSON.stringify(searchIndex));
console.log(
  `Built ${pages.length} documentation pages from Markdown into ${relative(root, out)}.`,
);

const overview = `# RavenRS

> RavenRS is an open-source Rust GUI framework: one declarative, reactive UI for Windows, macOS, Linux and WebAssembly, with GPU rendering, semantic design tokens, records, forms and reporting.

Framework version: ${version}. License: MIT OR Apache-2.0.

Start with \`npx create-ravenrs minha-loja\`. The npm preparer pins its Rust CLI release independently of npm-only patches. For exact APIs, check the project's Cargo.toml and use its version-matched RavenRS MCP server (\`cargo ravenrs mcp\`). Do not infer APIs from other GUI frameworks.

## Official guides

`;
const guideLinks = documents.map(({title, slug}) => `- [${title}](${siteUrl}/docs/${slug ? `${slug}/` : ""}index.md)`).join("\n");
const links = `

## Source and tools

- [Repository](https://github.com/alexkads/RavenRS)
- [Published Rust API](https://docs.rs/ravenrs/${version}/ravenrs/)
- [npm preparer](https://www.npmjs.com/package/create-ravenrs)
- [Combined guides](${siteUrl}/llms-full.txt)

These files provide readable context; they do not guarantee AI indexing or training.
`;
await writeFile(resolve(out, "../llms.txt"), overview + guideLinks + links);
await writeFile(resolve(out, "../llms-full.txt"), overview + guideLinks + links + documents.map(({title, slug, source, raw}) => `\n\n---\n\n# ${title}\n\nURL: ${siteUrl}/docs/${slug ? `${slug}/` : ""}\nSource: https://github.com/alexkads/RavenRS/blob/master/${source}\n\n${raw}`).join(""));
await writeFile(resolve(out, "../sitemap.xml"), `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">${documents.map(({slug}) => `<url><loc>${escape(`${siteUrl}/docs/${slug ? `${slug}/` : ""}`)}</loc></url>`).join("")}</urlset>\n`);
