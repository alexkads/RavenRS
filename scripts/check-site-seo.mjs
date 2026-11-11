// Validate the generated HTML, not just the generator's configuration.
import { readFile } from "node:fs/promises";
import { resolve, join } from "node:path";
import { parse, walk, attribute, text, pageRoute } from "./seo/html.mjs";
const out = resolve(process.argv[2] || "target/site-preview");
const manifest = JSON.parse(
  await readFile(join(out, "seo-manifest.json"), "utf8"),
);
const errors = [];
const check = (ok, file, message) => {
  if (!ok) errors.push(`${file}: ${message}`);
};
const images = new Set();
for (const page of manifest.pages) {
  const doc = parse(await readFile(join(out, page.file), "utf8"));
  const nodes = (predicate) => walk(doc, predicate);
  const tags = (key) =>
    nodes(
      (n) =>
        n.tagName === "meta" &&
        (attribute(n, "name") === key || attribute(n, "property") === key),
    );
  const value = (key) => attribute(tags(key)[0], "content");
  const title = nodes((n) => n.tagName === "title");
  const canonical = nodes(
    (n) => n.tagName === "link" && attribute(n, "rel") === "canonical",
  );
  const url = `${manifest.canonicalBase}/${pageRoute(page.file)}`;
  check(
    title.length === 1 && text(title[0]).trim() === page.title,
    page.file,
    "missing or inconsistent title",
  );
  check(
    tags("description").length === 1 &&
      value("description") === page.description &&
      !!page.description.trim(),
    page.file,
    "missing or inconsistent description",
  );
  check(
    canonical.length === 1 && attribute(canonical[0], "href") === url,
    page.file,
    "missing or inconsistent canonical",
  );
  for (const key of [
    "og:title",
    "og:description",
    "og:type",
    "og:url",
    "og:image",
    "og:image:alt",
    "twitter:title",
    "twitter:description",
    "twitter:card",
    "twitter:image",
    "twitter:image:alt",
  ]) {
    check(
      tags(key).length === 1 && !!value(key)?.trim(),
      page.file,
      `missing ${key}`,
    );
  }
  check(
    value("og:url") === url &&
      value("og:title") === page.title &&
      value("og:description") === page.description,
    page.file,
    "Open Graph does not match document",
  );
  check(
    value("twitter:card") === "summary_large_image" &&
      value("twitter:image") === page.image &&
      value("og:image") === page.image,
    page.file,
    "incorrect social image/card",
  );
  check(
    value("og:image:width") === "1200" && value("og:image:height") === "630",
    page.file,
    "incorrect image dimensions",
  );
  images.add(page.image);
  const structured = nodes(
    (n) =>
      n.tagName === "script" && attribute(n, "type") === "application/ld+json",
  );
  try {
    const data = JSON.parse(text(structured[0]));
    check(
      structured.length === 1 &&
        data["@context"] === "https://schema.org" &&
        data["@graph"].some(
          (n) =>
            n["@type"] === "SoftwareSourceCode" &&
            n.version === manifest.version,
        ),
      page.file,
      "invalid source schema",
    );
    if (page.route === "")
      check(
        data["@graph"].some((n) => n["@type"] === "SoftwareApplication"),
        page.file,
        "missing application schema",
      );
  } catch {
    check(false, page.file, "invalid JSON-LD");
  }
  if (nodes((n) => n.tagName === "canvas").length) {
    const intro = nodes(
      (n) => attribute(n, "data-raven-intro") !== undefined,
    )[0];
    check(
      intro &&
        walk(intro, (n) => n.tagName === "h1").length &&
        walk(intro, (n) => n.tagName === "a").length >= manifest.components &&
        attribute(intro, "hidden") === undefined,
      page.file,
      "missing readable HTML fallback",
    );
  }
}
for (const image of images) {
  check(
    image.startsWith(manifest.canonicalBase + "/"),
    image,
    "image outside canonical host",
  );
  const data = await readFile(
    join(out, image.slice(manifest.canonicalBase.length + 1)),
  );
  check(
    data
      .subarray(0, 8)
      .equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10])) &&
      data.readUInt32BE(16) === 1200 &&
      data.readUInt32BE(20) === 630,
    image,
    "not a 1200×630 PNG",
  );
}
const sitemap = await readFile(join(out, "sitemap.xml"), "utf8");
const locations = [...sitemap.matchAll(/<loc>(.*?)<\/loc>/g)].map((m) =>
  m[1].replaceAll("&amp;", "&"),
);
const expected = manifest.pages
  .filter((p) => p.indexed)
  .map((p) => p.canonical);
check(
  new Set(locations).size === locations.length &&
    locations.length === expected.length &&
    expected.every((url) => locations.includes(url)),
  "sitemap.xml",
  "missing or duplicate canonical URLs",
);
const robots = await readFile(join(out, "robots.txt"), "utf8");
check(
  robots.includes("Allow: /") &&
    robots.includes(`Sitemap: ${manifest.siteBase}/sitemap.xml`),
  "robots.txt",
  "wrong crawl policy or sitemap",
);
const config = JSON.parse(await readFile(join(out, "vercel.json"), "utf8"));
for (const example of manifest.examples) {
  const route = `playground/examples/${example.slug}/`;
  const page = manifest.pages.find((p) => p.route === route);
  check(page?.title.includes(example.name), route, "missing example metadata");
  check(
    config.rewrites.some(
      (r) =>
        r.has?.some((h) => h.key === "example" && h.value === example.name) &&
        r.destination === `/${route}index.html`,
    ),
    route,
    "missing query rewrite",
  );
}
if (errors.length) {
  console.error(errors.slice(0, 50).join("\n"));
  throw new Error(`${errors.length} SEO errors`);
}
console.log(
  `SEO check passed: ${manifest.pages.length} pages, ${locations.length} sitemap URLs, ${images.size} PNG images.`,
);
