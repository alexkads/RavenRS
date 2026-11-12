# Site SEO and publication

The canonical origin is **https://ravenrs.vercel.app**. GitHub Pages at
https://alexkads.github.io/RavenRS/ is a mirror: each page names the equivalent
Vercel URL as its canonical. Keep both origins accessible; no custom domain is required.

## Build and validate locally

```bash
./scripts/build-site.sh /tmp/ravenrs-site
node scripts/check-site-seo.mjs /tmp/ravenrs-site
node scripts/test-site-seo.mjs /tmp/ravenrs-site
```

`build-site.sh` runs the SEO generator and its validation gate after assembling
Markdown, component pages, API documentation and the WebAssembly applications.
Missing titles, descriptions, canonicals, social cards or structured data stop the
build. The regression test removes required metadata from a real built page and
checks that publication is rejected.

Set `RAVENRS_SITE_URL` to the deployment origin (including `/RavenRS` for Pages).
`RAVENRS_CANONICAL_URL` defaults to the primary Vercel origin; change it only when
moving the entire site's canonical policy. Both hosts' sitemaps list canonical
URLs. `seo-manifest.json` records all generated routes, release and metadata.

For a metadata-only deployment, reuse a trusted, published build of the **same
workspace release**, rather than rebuilding Rust and WebAssembly:

```bash
RAVENRS_SITE_ASSETS_FROM=/tmp/published-gh-pages \
  ./scripts/build-site.sh /tmp/ravenrs-site
```

The input must contain `api/`, the home WASM and gallery WASM. The builder checks
the API release and refuses an output nested inside the input. This option is
not for publishing changes to Rust applications or framework code. Obtain the
input from the repository's `gh-pages` branch; never use an untrusted artifact.

## Crawlable pages and examples

Canvas pages contain an ordinary HTML heading, summary and component links.
Playground examples also contain their Rust source. This content is readable
without JavaScript and stays available if a GPU backend cannot start. It hides
when the interactive canvas initializes.

Every example has a static route, such as `/playground/examples/button/`, with
its own title, description and social PNG. Existing `/playground/?example=Button`
links remain usable: Vercel permanently redirects to that example's static route, while
Pages navigates to its static route in the browser. Share static routes for
previews on both hosts; Pages cannot vary its initial HTML by query string.

API canonicals and sitemap entries use `/api/`. Vercel stores those files in
`reference/` and rewrites requests; this storage detail is never a canonical URL.
Rustdoc source listings and implementation indexes are omitted from the sitemap.

## Sharing and structured data

The generator produces 1200×630 PNGs for the home, each component, each example
and the main sections. It uses bundled Inter fonts and `@resvg/resvg-js`, without
system fonts or remote rendering services. `parse5` edits and validates actual
HTML, including rustdoc. Both are private build dependencies, not SDK or CLI
runtime dependencies.

All pages have Open Graph metadata and a Twitter `summary_large_image` card.
Home JSON-LD describes the project as `SoftwareSourceCode` and
`SoftwareApplication`; other pages describe their article and the source project.
There are no fabricated reviews, ratings or indexing guarantees.

After deployment, check the initial HTML, image HTTP status and actual PNG
size for the home, a component and an example. Check API canonicals too. Use
[Schema.org Validator](https://validator.schema.org/) for structured data and an
Open Graph inspector for social previews. Social networks may cache old cards.

## Robots and publishing without Actions

The Vercel root `/robots.txt` references its sitemap. Pages also ships
`/RavenRS/robots.txt`, but crawlers consult **https://alexkads.github.io/robots.txt**.
That root file lives in `alexkads/alexkads.github.io` and must retain a
`Sitemap: https://alexkads.github.io/RavenRS/sitemap.xml` entry alongside other
projects' sitemaps.

Publish Pages as static files on `gh-pages` with `.nojekyll`, and deploy Vercel
with `./scripts/deploy-vercel.sh --prod`. GitHub Actions remain disabled; all
builds and checks run locally. A site metadata change does not require an npm
or crates.io release.
