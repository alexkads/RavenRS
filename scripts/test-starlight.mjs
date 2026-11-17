// Verify the real generated portal, including both hosting base paths.
import assert from "node:assert/strict";
import { readFile, access } from "node:fs/promises";
import { resolve, join } from "node:path";
import { test } from "node:test";
import { root, pages } from "../site/src/lib/content.mjs";
import { publicPath, siteUrl } from "../site/src/lib/site.mjs";
import { parse, walk, attribute, text } from "./seo/html.mjs";

const out = resolve(root, process.argv[2] || "site/dist");
const html = async (path) => parse(await readFile(join(out, path, "index.html"), "utf8"));
const nodes = (doc, tag) => walk(doc, (node) => node.tagName === tag);

test("Astro owns the home page and links to guides and interactive examples", async () => {
  const home = await html("");
  assert.match(text(nodes(home, "h1")[0]), /One UI\. Native and web\./);
  assert.equal(nodes(home, "canvas").length, 0);
  const links = nodes(home, "a").map((node) => attribute(node, "href"));
  for (const route of ["docs/", "docs/getting-started/", "components/", "playground/", "api/ravenrs/", "designer/"])
    assert.ok(links.includes(publicPath(route)), `Missing home link to ${route}`);
});

test("Every repository guide has HTML, a source link and unchanged raw Markdown", async () => {
  const llms = await readFile(join(out, "llms.txt"), "utf8");
  const full = await readFile(join(out, "llms-full.txt"), "utf8");
  for (const page of pages) {
    const doc = await html(page.slug);
    assert.equal(nodes(doc, "h1").length, 1, page.source);
    const raw = await readFile(join(out, page.slug, "index.md"), "utf8");
    assert.equal(raw, (await readFile(resolve(root, page.source), "utf8")).replace(/^---\r?\n[\s\S]*?\r?\n---\r?\n/, ""));
    assert.ok(nodes(doc, "a").some((node) => attribute(node, "href") === `https://github.com/alexkads/RavenRS/edit/master/${page.source}`));
    assert.ok(nodes(doc, "a").some((node) => attribute(node, "href") === "index.md"));
    assert.ok(llms.includes(`${siteUrl}/${page.slug.replace(/\/$/, "")}/index.md`));
    assert.ok(full.includes(raw));
  }
});

test("Internal guide links, images, styles and scripts resolve for this host", async () => {
  const prefix = new URL(siteUrl).pathname.replace(/\/$/, "");
  for (const page of [{ slug: "" }, ...pages]) {
    const doc = await html(page.slug);
    for (const node of walk(doc, (node) => ["a", "img", "script", "link"].includes(node.tagName))) {
      const value = attribute(node, node.tagName === "img" || node.tagName === "script" ? "src" : "href");
      if (!value?.startsWith("/") || value.startsWith("//")) continue;
      assert.ok(value.startsWith(`${prefix}/`), `${page.slug}: ${value} omits host base ${prefix}`);
      const path = decodeURI(value.slice(prefix.length + 1).split(/[?#]/)[0]);
      if (!path || /^(?:docs\/|_astro\/|pagefind\/|favicon)/.test(path)) {
        await access(join(out, path, path.endsWith("/") || !path ? "index.html" : ""));
      }
    }
  }
  const gettingStarted = await html("docs/getting-started");
  assert.ok(nodes(gettingStarted, "img").some((node) => attribute(node, "src") === publicPath("docs/_media/create-ravenrs-cli.png")));
  const architecture = await html("docs/architecture");
  assert.ok(walk(architecture, (node) => attribute(node, "id") === "6-5-motion").length, "Legacy numbered anchors must survive");
});

test("Pagefind ships a search index covering every portal page", async () => {
  await access(join(out, "pagefind/pagefind.js"));
  const index = JSON.parse(await readFile(join(out, "pagefind/pagefind-entry.json"), "utf8"));
  assert.equal(Object.values(index.languages).reduce((count, lang) => count + lang.page_count, 0), pages.length + 1);
  for (const lang of Object.values(index.languages)) {
    await access(join(out, `pagefind/pagefind.${lang.hash}.pf_meta`));
    await access(join(out, `pagefind/wasm.${lang.wasm}.pagefind`));
  }
});
