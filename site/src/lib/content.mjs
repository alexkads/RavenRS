import { readFile, writeFile, mkdir, readdir, rm, cp } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { resolve, dirname, posix } from "node:path";
import { corePages } from "./pages.mjs";
import { siteUrl, publicPath } from "./site.mjs";

export const root = fileURLToPath(new URL("../../../", import.meta.url));
export const project = resolve(root, "site");
export const version = (await readFile(resolve(root, "Cargo.toml"), "utf8")).match(/\[workspace\.package\][\s\S]*?version = "([^"]+)"/)[1];

async function markdownFiles(folder) {
  const result = [];
  for (const entry of await readdir(resolve(root, folder), { withFileTypes: true })) {
    // The tutorial's sample project is source code, not part of the guides.
    if (["media", "projeto", "output"].includes(entry.name)) continue;
    const path = posix.join(folder, entry.name);
    if (entry.isDirectory()) result.push(...await markdownFiles(path));
    else if (entry.name.endsWith(".md")) result.push(path);
  }
  return result.sort();
}

export const pages = corePages.map(([group, slug, title, source]) => ({ group, slug: `docs/${slug}`, title, source }));
const known = new Set(pages.map((p) => p.source));
for (const source of await markdownFiles("docs")) {
  if (known.has(source)) continue;
  const raw = await readFile(resolve(root, source), "utf8");
  const title = raw.match(/^#\s+(.+)$/m)?.[1] || posix.basename(source, ".md");
  const slug = source.replace(/\.md$/, "").replace(/_/g, "-").toLowerCase().replace(/\/(?:readme|00-readme)$/, "");
  const group = source.startsWith("docs/data-studio/") ? "Data Studio" : source.startsWith("docs/tutorials/") ? "Tutorials" : "Technical guides";
  pages.push({ group, slug, title, source });
}
if (new Set(pages.map((p) => p.slug.replace(/\/$/, ""))).size !== pages.length) throw new Error("Duplicate documentation route");

export async function prepareContent({ clean = true } = {}) {
  const content = resolve(project, "src/content/docs");
  const assets = resolve(project, ".generated/public");
  if (clean) {
    await rm(content, { recursive: true, force: true });
    await rm(assets, { recursive: true, force: true });
  }
  await mkdir(content, { recursive: true });
  await mkdir(assets, { recursive: true });
  await cp(resolve(project, "public"), assets, { recursive: true });
  await cp(resolve(project, "src/assets/favicon.svg"), resolve(assets, "favicon.svg"));
  await cp(resolve(root, "docs/media"), resolve(assets, "docs/_media"), { recursive: true });
  const documents = [];
  for (const page of pages) {
    const raw = (await readFile(resolve(root, page.source), "utf8")).replace(/^---\r?\n[\s\S]*?\r?\n---\r?\n/, "");
    const target = resolve(content, `${page.slug.replace(/\/$/, "/index")}.md`);
    await mkdir(dirname(target), { recursive: true });
    const data = {
      title: page.title,
      description: `${page.title} — the RavenRS guide to native and web applications in Rust.`,
      source: page.source,
      frameworkVersion: version,
      editUrl: `https://github.com/alexkads/RavenRS/edit/master/${page.source}`,
    };
    // Starlight supplies the page's h1; preserve the repository Markdown unchanged.
    const body = raw.replace(/^#\s+[^\n]+\n/, "");
    await writeFile(target, `---\n${Object.entries(data).map(([key, value]) => `${key}: ${JSON.stringify(value)}`).join("\n")}\n---\n${body}`);
    const markdown = resolve(assets, page.slug, "index.md");
    await mkdir(dirname(markdown), { recursive: true });
    await writeFile(markdown, raw);
    documents.push({ ...page, raw });
  }
  await writeFile(resolve(content, "index.mdx"), `---
title: RavenRS
description: Build native desktop and browser applications with one declarative, reactive UI in Rust.
template: splash
hero:
  title: One UI. Native and web.
  tagline: Build applications in Rust with GPU rendering, reactive state, semantic design tokens, forms and reporting.
  actions:
    - text: Get started
      link: ${publicPath("docs/getting-started/")}
      icon: right-arrow
    - text: Explore components
      link: ${publicPath("components/")}
      variant: secondary
    - text: GitHub
      link: https://github.com/alexkads/RavenRS
      variant: minimal
---
import { Card, CardGrid, LinkCard } from '@astrojs/starlight/components';

## Start building

\`\`\`bash
npx create-ravenrs minha-loja
\`\`\`

RavenRS ${version} · MIT OR Apache-2.0 · Windows, macOS, Linux and WebAssembly.

<CardGrid>
  <Card title="One declarative interface" icon="laptop">Compose typed Rust views for native windows and browser canvases, using the same components and layout engine.</Card>
  <Card title="Reactive by design" icon="rocket">Connect state to your views and use semantic design tokens across light and dark themes.</Card>
  <Card title="Build applications" icon="setting">Create records, forms, lists and application workflows with the RavenRS application framework.</Card>
  <Card title="Reports and exports" icon="document">Design reports, render documents and export to PDF, XLSX and CSV.</Card>
</CardGrid>

## Explore RavenRS

<CardGrid>
  <LinkCard title="Documentation" description="Installation, architecture, application guides and reference." href="${publicPath("docs/")}" />
  <LinkCard title="Playground" description="Explore interactive components and Rust examples." href="${publicPath("playground/")}" />
  <LinkCard title="Rust API" description="Browse the generated API for this framework release." href="${publicPath("api/ravenrs/")}" />
  <LinkCard title="Report designer" description="Open the interactive report designer." href="${publicPath("designer/")}" />
</CardGrid>
`);
  const overview = `# RavenRS\n\n> An open-source Rust GUI framework: one declarative, reactive UI for Windows, macOS, Linux and WebAssembly, with GPU rendering, semantic design tokens, records, forms and reporting.\n\nFramework version: ${version}. License: MIT OR Apache-2.0.\n\nStart with \`npx create-ravenrs minha-loja\`. For exact APIs, check the project's Cargo.toml and use its version-matched RavenRS MCP server (\`cargo ravenrs mcp\`).\n\n## Official guides\n\n`;
  const links = documents.map(({ title, slug }) => `- [${title}](${siteUrl}/${slug.replace(/\/$/, "")}/index.md)`).join("\n");
  const tools = `\n\n## Source and tools\n\n- [Repository](https://github.com/alexkads/RavenRS)\n- [Published Rust API](https://docs.rs/ravenrs/${version}/ravenrs/)\n- [npm preparer](https://www.npmjs.com/package/create-ravenrs)\n- [Combined guides](${siteUrl}/llms-full.txt)\n\nThese files provide readable context; they do not guarantee AI indexing or training.\n`;
  await writeFile(resolve(assets, "llms.txt"), overview + links + tools);
  await writeFile(resolve(assets, "llms-full.txt"), overview + links + tools + documents.map(({ title, slug, source, raw }) => `\n\n---\n\n# ${title}\n\nURL: ${siteUrl}/${slug.replace(/\/$/, "")}/\nSource: https://github.com/alexkads/RavenRS/blob/master/${source}\n\n${raw}`).join(""));
}
