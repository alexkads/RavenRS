// Exercise the build gate against a real generated page, with deliberate regressions.
import assert from "node:assert/strict";
import {
  readFile,
  writeFile,
  mkdir,
  mkdtemp,
  copyFile,
  rm,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve, dirname } from "node:path";
import { spawnSync } from "node:child_process";
import {
  parse,
  serialize,
  walk,
  attribute,
  remove,
  pageRoute,
} from "./seo/html.mjs";
const source = resolve(process.argv[2] || "target/site-preview");
const fixture = await mkdtemp(join(tmpdir(), "ravenrs-seo-regression-"));
try {
  const manifest = JSON.parse(
    await readFile(join(source, "seo-manifest.json"), "utf8"),
  );
  const home = manifest.pages.find((p) => p.route === "");
  const original = await readFile(join(source, home.file), "utf8");
  manifest.pages = [home];
  manifest.examples = [];
  await writeFile(join(fixture, "seo-manifest.json"), JSON.stringify(manifest));
  for (const file of ["robots.txt", "vercel.json"])
    await copyFile(join(source, file), join(fixture, file));
  await writeFile(
    join(fixture, "sitemap.xml"),
    `<urlset><url><loc>${home.canonical}</loc></url></urlset>`,
  );
  const image = home.image.slice(manifest.canonicalBase.length + 1);
  await mkdir(dirname(join(fixture, image)), { recursive: true });
  await copyFile(join(source, image), join(fixture, image));
  const run = () =>
    spawnSync(process.execPath, ["scripts/check-site-seo.mjs", fixture], {
      encoding: "utf8",
    });
  await writeFile(join(fixture, home.file), original);
  assert.equal(run().status, 0, "a real generated page must pass");
  for (const key of ["title", "description", "canonical", "og:image"]) {
    const doc = parse(original);
    walk(doc, (n) =>
      key === "title"
        ? n.tagName === "title"
        : key === "canonical"
          ? n.tagName === "link" && attribute(n, "rel") === key
          : n.tagName === "meta" &&
            (attribute(n, "name") === key || attribute(n, "property") === key),
    ).forEach(remove);
    await writeFile(join(fixture, home.file), serialize(doc));
    const result = run();
    assert.notEqual(result.status, 0, `removing ${key} must block deployment`);
    assert.ok(result.stderr.includes(key), result.stderr);
  }
  assert.equal(pageRoute("reference/ravenrs/index.html"), "api/ravenrs/");
  assert.equal(
    pageRoute("api/ravenrs/struct.Button.html"),
    "api/ravenrs/struct.Button.html",
  );
  console.log(
    "SEO regression tests passed: missing title, description, canonical and social image block deployment; API aliases keep /api/.",
  );
} finally {
  await rm(fixture, { recursive: true, force: true });
}
