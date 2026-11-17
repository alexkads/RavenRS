# RavenRS website — Astro + Starlight

The home page and documentation are built with Astro and Starlight. The same
static portal is deployed to Vercel and GitHub Pages. Interactive Rust examples,
component pages and rustdoc keep their existing routes.

The project follows the standard Astro layout:

```text
site/
  src/        Theme, components, content collection and Markdown adapters
  public/     Static assets, component catalog and playground
  astro.config.mjs
  package.json
  vercel.json
```

Generated content, build output and dependencies are ignored by Git.

Requires Node.js 22.23.4 or later (Node 24 recommended).

```bash
npm ci --prefix site --ignore-scripts
npm run dev --prefix site
```

Edit the original Markdown in `docs/`, `ARCHITECTURE.md` or the CLI README.
`src/lib/content.mjs` prepares Starlight content from these sources; generated files
under `src/content/docs/` and `.generated/public/` are ignored. Existing source edits refresh
the development server. Restart it after adding or removing guides. The original
13 guide URLs and heading anchors are preserved, and the remaining guides under
`docs/` are included in the sidebar and Pagefind search. Sample projects and
generated tutorial output are excluded. Source code links open GitHub.

`src/lib/pages.mjs` defines the curated navigation and established routes.
`astro.config.mjs` configures Starlight, and `src/styles/custom.css` contains the
RavenRS theme. The generated home page uses Starlight's splash layout and cards.

```bash
npm run check --prefix site
node scripts/build-docs.mjs target/portal-vercel
npm test --prefix site -- target/portal-vercel

RAVENRS_SITE_URL=https://alexkads.github.io/RavenRS \
  node scripts/build-docs.mjs target/portal-pages
RAVENRS_SITE_URL=https://alexkads.github.io/RavenRS \
  npm test --prefix site -- target/portal-pages
```

These portal previews omit the Rust applications and API files. For the complete
site, use `scripts/build-site.sh <output>`. It merges the Astro portal with those
applications, then generates and validates canonical metadata, social images,
structured data and a combined sitemap. See `docs/SITE_SEO.md` for asset reuse.

`scripts/deploy-vercel.sh --prod` uses the domain root; `scripts/deploy-pages.sh`
uses `/RavenRS/`. Both run the same build. `RAVENRS_SITE_URL` controls asset and
navigation paths; `RAVENRS_CANONICAL_URL` controls the complete site's canonical
origin. Raw `index.md`, `llms.txt` and `llms-full.txt` are also generated.
