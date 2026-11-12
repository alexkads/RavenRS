// Apply one canonical policy to every public HTML page, including generated rustdoc.
import {
  readFile,
  writeFile,
  mkdir,
  copyFile,
  readdir,
} from "node:fs/promises";
import { resolve, dirname, relative, join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  parse,
  serialize,
  parseFragment,
  walk,
  attribute,
  attr,
  text,
  element,
  append,
  remove,
  meta,
  setMeta,
  plain,
  pageRoute,
} from "./seo/html.mjs";
import { socialImage } from "./seo/social.mjs";
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const out = resolve(process.argv[2] || "target/site-preview");
const canonicalBase = (
  process.env.RAVENRS_CANONICAL_URL || "https://ravenrs.vercel.app"
).replace(/\/$/, "");
const siteBase = (
  process.env.RAVENRS_SITE_URL || "https://alexkads.github.io/RavenRS"
).replace(/\/$/, "");
for (const url of [canonicalBase, siteBase])
  if (!/^https?:\/\//.test(url))
    throw new Error("Absolute HTTP site URLs required");
const prefix = new URL(siteBase).pathname.replace(/\/$/, "");
const publicPath = (path) => `${prefix}/${path}`;
const canonical = (path) => `${canonicalBase}/${path}`;
const version = (await readFile(join(root, "Cargo.toml"), "utf8")).match(
  /\[workspace\.package\][\s\S]*?version = "([^"]+)"/,
)[1];
const esc = (s) =>
  String(s)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
const catalog = parse(
  await readFile(join(root, "site/components/index.html"), "utf8"),
);
const components = walk(
  catalog,
  (n) => n.tagName === "a" && attribute(n, "data-api"),
).map((a) => {
  const li = a.parentNode;
  const desc = walk(li, (n) => attribute(n, "class") === "desc")[0];
  return {
    name: plain(text(a)),
    slug: attribute(a, "href").replace(/\/$/, ""),
    description: plain(text(desc)),
    api: attribute(a, "data-api"),
  };
});
if (!components.length) throw new Error("Missing component catalog");
const bySlug = new Map(components.map((c) => [c.slug, c]));
const samples = [];
for (const file of await readdir(
  join(root, "examples/component-gallery/src/samples"),
)) {
  if (!file.endsWith(".rs")) continue;
  const source = await readFile(
    join(root, "examples/component-gallery/src/samples", file),
    "utf8",
  );
  for (const match of source.matchAll(
    /^\/\/ sample: (\w+), \d+\n([\s\S]*?)^\/\/ end$/gm,
  ))
    samples.push({
      name: match[1],
      code: match[2].trim(),
      slug: match[1].toLowerCase(),
    });
}
samples.sort((a, b) => a.name.localeCompare(b.name));
if (new Set(samples.map((s) => s.slug)).size !== samples.length)
  throw new Error("Duplicate playground route");
const exampleRoutes = Object.fromEntries(
  samples.map((s) => [s.name, `examples/${s.slug}/`]),
);
await mkdir(join(out, "assets/seo"), { recursive: true });
for (const file of ["canvas.js", "playground.js"])
  await copyFile(join(root, "site/seo", file), join(out, "assets/seo", file));
const playground = await readFile(join(out, "playground/index.html"), "utf8");
for (const sample of samples) {
  const folder = join(out, "playground/examples", sample.slug);
  await mkdir(folder, { recursive: true });
  const doc = parse(playground);
  const head = walk(doc, (n) => n.tagName === "head")[0];
  append(head, element("base", { href: "../../" }));
  append(
    head,
    element("meta", { name: "ravenrs-example", content: sample.name }),
  );
  await writeFile(join(folder, "index.html"), serialize(doc));
}
const general = {
  "": {
    title: "RavenRS — Rust GUI framework for desktop and web",
    description: `Build declarative Rust interfaces for Windows, macOS, Linux and WebAssembly. Explore ${components.length} components, reactive state, business apps and reporting.`,
    heading: "One Rust UI. Native and web.",
    section: "Rust GUI framework",
    image: "home",
  },
  "demo/": {
    title: "Rust GUI component gallery — RavenRS",
    description: `Explore ${components.length} RavenRS GUI components running on the GPU. Try buttons, forms, charts and data grids for Rust desktop and web apps.`,
    heading: "Explore the live component gallery",
    section: "Component gallery",
    image: "gallery",
  },
  "playground/": {
    title: "Rust GUI playground — RavenRS examples",
    description:
      "Edit real RavenRS examples in Rust, preview the interface in your browser and download a Cargo project for desktop or WebAssembly.",
    heading: "Build your next Rust interface",
    section: "Interactive playground",
    image: "playground",
  },
  "designer/": {
    title: "Visual report designer for Rust — RavenRS",
    description:
      "Design report layouts, bind data and explore reporting for Rust desktop and browser applications with the RavenRS report designer.",
    heading: "Design reports with RavenRS",
    section: "Report designer",
    image: "reports",
  },
  "hello/": {
    title: "Hello RavenRS — Rust GUI starter example",
    description:
      "Try the RavenRS starter application: a reusable card, reactive click counter and buttons, built with one Rust UI for desktop and the browser.",
    heading: "Start with a small Rust interface",
    section: "Hello, RavenRS",
    image: "hello",
  },
  "quick/": {
    title: "Model-driven Rust business applications — RavenRS",
    description:
      "Explore lists, forms, navigation and business rules generated from Rust entity metadata with ravenrs-app.",
    heading: "Business apps from Rust metadata",
    section: "Application framework",
    image: "applications",
  },
  "components/": {
    title: "Rust GUI components and examples — RavenRS",
    description: `Browse ${components.length} Rust GUI components with live examples, source code and API documentation for native desktop and WebAssembly.`,
    heading: "Rust GUI components",
    section: "Component reference",
    image: "components",
  },
};
const imageCache = new Map();
async function imageFor(info) {
  if (!imageCache.has(info.image))
    imageCache.set(
      info.image,
      await socialImage(
        out,
        info.image,
        info.heading || info.title.replace(/ · RavenRS$/, ""),
        info.description,
        info.section,
      ),
    );
  return imageCache.get(info.image);
}
async function htmlFiles(folder) {
  const files = [];
  for (const entry of await readdir(folder, { withFileTypes: true })) {
    if (entry.name.startsWith(".")) continue;
    const path = join(folder, entry.name);
    if (entry.isDirectory()) files.push(...(await htmlFiles(path)));
    else if (entry.name.endsWith(".html")) files.push(path);
  }
  return files;
}
const records = [];
for (const file of await htmlFiles(out)) {
  const route = pageRoute(relative(out, file));
  const doc = parse(await readFile(file, "utf8"));
  const head = walk(doc, (n) => n.tagName === "head")[0];
  const body = walk(doc, (n) => n.tagName === "body")[0];
  if (!head || !body) throw new Error("Invalid document: " + route);
  for (const node of walk(
    doc,
    (n) =>
      attribute(n, "data-raven-seo") !== undefined ||
      attribute(n, "id") === "raven-example-routes" ||
      (n.tagName === "script" &&
        /\/assets\/seo\//.test(attribute(n, "src") || "")),
  ))
    remove(node);
  const titleNode = walk(head, (n) => n.tagName === "title")[0];
  let info = general[route];
  const component = bySlug.get(route.match(/^components\/([^/]+)\/$/)?.[1]);
  const sample = samples.find(
    (s) => route === `playground/examples/${s.slug}/`,
  );
  if (component)
    info = {
      title: `${component.name} — Rust GUI component · RavenRS`,
      description: `${component.description}. Explore a live ${component.name} example, Rust source and API documentation for desktop and WebAssembly.`,
      heading: component.name,
      section: "Rust GUI component",
      image: "component-" + component.slug,
    };
  if (sample)
    setMeta(
      head,
      "ravenrs-example-url",
      publicPath(`playground/examples/${sample.slug}/`),
    );
  if (sample)
    info = {
      title: `${sample.name} — Rust GUI playground · RavenRS`,
      description: `Edit and run the ${sample.name} RavenRS example in your browser. Explore its Rust code and download a Cargo project for desktop and WebAssembly.`,
      heading: sample.name,
      section: "Interactive Rust example",
      image: "example-" + sample.slug,
    };
  if (!info) {
    const original = titleNode ? plain(text(titleNode)) : "";
    const heading = walk(body, (n) => n.tagName === "h1")[0];
    const title =
      original ||
      `${heading ? plain(text(heading)) : "API reference"} · RavenRS`;
    let description = attribute(meta(head, "description"), "content");
    if (!description?.trim())
      description = `${title}. Reference and examples for the RavenRS Rust GUI framework, native desktop and WebAssembly.`;
    info = {
      title,
      description: plain(description).slice(0, 220),
      heading: route.startsWith("api/")
        ? "Rust API reference"
        : title.split(" · ")[0],
      section: route.startsWith("api/")
        ? "Framework API"
        : "RavenRS documentation",
      image: route.startsWith("api/") ? "api" : "documentation",
    };
  }
  if (titleNode) {
    titleNode.childNodes = [];
    append(titleNode, { nodeName: "#text", value: info.title });
  } else append(head, element("title", {}, info.title));
  setMeta(head, "description", info.description);
  for (const node of walk(
    head,
    (n) => n.tagName === "link" && attribute(n, "rel") === "canonical",
  ))
    remove(node);
  const url = canonical(route);
  append(head, element("link", { rel: "canonical", href: url }));
  const image = canonical(await imageFor(info));
  for (const [key, value] of Object.entries({
    "og:title": info.title,
    "og:description": info.description,
    "og:type": route ? "article" : "website",
    "og:url": url,
    "og:site_name": "RavenRS",
    "og:locale": "en_US",
    "og:image": image,
    "og:image:secure_url": image,
    "og:image:type": "image/png",
    "og:image:width": "1200",
    "og:image:height": "630",
    "og:image:alt": `${info.heading} — RavenRS Rust GUI framework`,
  }))
    setMeta(head, key, value, true);
  for (const [key, value] of Object.entries({
    "twitter:card": "summary_large_image",
    "twitter:title": info.title,
    "twitter:description": info.description,
    "twitter:image": image,
    "twitter:image:alt": `${info.heading} — RavenRS`,
  }))
    setMeta(head, key, value);
  for (const node of walk(
    head,
    (n) =>
      n.tagName === "script" && attribute(n, "type") === "application/ld+json",
  ))
    remove(node);
  const software = {
    "@type": "SoftwareSourceCode",
    "@id": canonical("") + "#source",
    name: "RavenRS",
    description: general[""].description,
    programmingLanguage: "Rust",
    codeRepository: "https://github.com/alexkads/RavenRS",
    version,
    license: [
      "https://github.com/alexkads/RavenRS/blob/master/LICENSE-MIT",
      "https://github.com/alexkads/RavenRS/blob/master/LICENSE-APACHE",
    ],
    url: canonical(""),
  };
  const graph = route
    ? [
        {
          "@type": "TechArticle",
          headline: info.title,
          description: info.description,
          url,
          version,
          about: { "@id": software["@id"] },
        },
        software,
      ]
    : [
        software,
        {
          "@type": "SoftwareApplication",
          name: "RavenRS",
          url: canonical(""),
          description: general[""].description,
          applicationCategory: "DeveloperApplication",
          operatingSystem: "Windows, macOS, Linux, Web",
          softwareVersion: version,
          offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
          downloadUrl: "https://www.npmjs.com/package/create-ravenrs",
        },
      ];
  append(
    head,
    element(
      "script",
      { type: "application/ld+json" },
      JSON.stringify({
        "@context": "https://schema.org",
        "@graph": graph,
      }).replaceAll("<", "\\u003c"),
    ),
  );
  const canvas = walk(body, (n) => n.tagName === "canvas")[0];
  if (canvas) {
    // Replace old noscript-only text with real progressive-enhancement content.
    walk(
      body,
      (n) =>
        n.tagName === "noscript" ||
        attribute(n, "data-raven-intro") !== undefined,
    ).forEach(remove);
    const choices = sample ? samples : components;
    const navigation = choices
      .map(
        (item) =>
          `<a href="${publicPath(sample ? `playground/examples/${item.slug}/` : `components/${item.slug}/`)}">${esc(item.name)}</a>`,
      )
      .join("");
    const code = sample
      ? `<h2>Rust source</h2><pre><code>${esc(sample.code)}</code></pre>`
      : "<pre><code>npx create-ravenrs minha-loja</code></pre>";
    const fragment = parseFragment(
      `<main data-raven-intro><div class="raven-intro-inner"><p class="raven-intro-brand">RavenRS · Rust GUI</p><h1>${esc(info.heading)}</h1><p>${esc(info.description)}</p><p>One declarative Rust interface, GPU rendering, reactive state and semantic design tokens. Free and open source under MIT OR Apache-2.0.</p><nav aria-label="Explore RavenRS"><a href="${publicPath("docs/getting-started/")}">Get started</a><a href="${publicPath("components/")}">Components</a><a href="${publicPath("playground/")}">Playground</a><a href="${publicPath("api/ravenrs/")}">Rust API</a></nav>${code}<p class="raven-loading" data-raven-loading>The interactive preview loads when JavaScript and WebGPU or WebGL2 are available. You can read the guide and examples here at any time.</p><h2>${sample ? "More examples" : "Explore the components"}</h2><div class="raven-intro-links">${navigation}</div></div></main>`,
    );
    for (const child of fragment.childNodes) append(body, child);
    append(
      head,
      element(
        "style",
        { "data-raven-seo": "" },
        `[data-raven-intro]{position:fixed;inset:0;overflow:auto;z-index:2;background:#0b0d12;color:#eef1f7;font:16px/1.65 system-ui,sans-serif;padding:32px 24px}[data-raven-intro][hidden]{display:none}.raven-intro-inner{max-width:1000px;margin:auto}.raven-intro-brand{color:#9aabff;font-weight:650}.raven-intro-inner h1{font-size:clamp(30px,5vw,54px);line-height:1.15}.raven-intro-inner nav,.raven-intro-links{display:flex;flex-wrap:wrap;gap:10px 20px}.raven-intro-inner a{color:#a5b8ff}.raven-intro-inner pre{white-space:pre-wrap;background:#171d2a;padding:18px;border-radius:12px}.raven-loading{color:#a5afc4} .raven-error{z-index:5}#opening{display:none!important}`,
      ),
    );
    append(
      body,
      element("script", {
        type: "module",
        src: publicPath("assets/seo/canvas.js"),
      }),
    );
  }
  if (route.startsWith("playground/")) {
    append(
      head,
      element(
        "script",
        { id: "raven-example-routes", type: "application/json" },
        JSON.stringify(exampleRoutes),
      ),
    );
    append(
      head,
      element("script", {
        type: "module",
        src: publicPath("assets/seo/playground.js"),
      }),
    );
    // Linkable static examples are discoverable even without running the canvas.
    if (!sample) {
      const intro = walk(
        body,
        (n) => attribute(n, "data-raven-intro") !== undefined,
      )[0];
      const list = parseFragment(
        `<section><h2>Playground examples</h2>${samples.map((item) => `<a href="${publicPath(`playground/examples/${item.slug}/`)}">${esc(item.name)}</a> `).join("")}</section>`,
      );
      for (const child of list.childNodes) append(intro, child);
    }
  }
  // Component links now share the static URL that has complete server-readable metadata.
  for (const a of walk(body, (n) => n.tagName === "a")) {
    const href = attribute(a, "href");
    const name = href?.match(/playground\/\?example=(\w+)/)?.[1];
    if (name && Object.hasOwn(exampleRoutes, name))
      attr(a, "href", publicPath("playground/" + exampleRoutes[name]));
  }
  await writeFile(file, serialize(doc));
  const indexed =
    !route.startsWith("api/src/") &&
    !route.endsWith("/all.html") &&
    !route.includes("/trait.impl/") &&
    !route.includes("/type.impl/");
  records.push({
    file: relative(out, file).replaceAll("\\", "/"),
    route,
    canonical: url,
    title: info.title,
    description: info.description,
    image,
    indexed,
  });
}
await writeFile(
  join(out, "seo-manifest.json"),
  JSON.stringify(
    {
      canonicalBase,
      siteBase,
      version,
      components: components.length,
      examples: samples.map(({ name, slug }) => ({ name, slug })),
      pages: records,
    },
    null,
    2,
  ),
);
const indexed = records.filter((p) => p.indexed);
if (indexed.length > 50000)
  throw new Error("Sitemap needs splitting: >50,000 URLs");
await writeFile(
  join(out, "sitemap.xml"),
  `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${indexed.map((p) => `<url><loc>${esc(p.canonical)}</loc></url>`).join("\n")}\n</urlset>\n`,
);
await writeFile(
  join(out, "robots.txt"),
  `User-agent: *\nAllow: /\n\nSitemap: ${siteBase}/sitemap.xml\n`,
);
// Redirect before the filesystem: rewrites cannot override playground/index.html.
const config = JSON.parse(
  await readFile(join(root, "site/vercel.json"), "utf8"),
);
config.redirects = [
  ...samples.map((s) => ({
    source: "/playground/",
    has: [{ type: "query", key: "example", value: s.name }],
    destination: `/playground/examples/${s.slug}/`,
    permanent: true,
  })),
  ...(config.redirects || []),
];
await writeFile(
  join(out, "vercel.json"),
  JSON.stringify(config, null, 2) + "\n",
);
await writeFile(join(out, ".nojekyll"), "");
console.log(
  `SEO: ${records.length} HTML pages, ${components.length} components, ${samples.length} examples, ${imageCache.size} social images. Canonical: ${canonicalBase}`,
);
