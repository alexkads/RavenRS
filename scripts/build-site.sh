#!/usr/bin/env bash
# Builds the whole site into a folder: the home page, the WebAssembly demos, the playground, the
# component pages and the API docs. The build uses the configured site origin so it serves at
# the root of a domain (Vercel) or under a path (GitHub Pages, /RavenRS/).
#
#   scripts/build-site.sh <folder>
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
site="${1:?usage: scripts/build-site.sh <folder>}"
mkdir -p "$site"
site="$(cd "$site" && pwd)"

# Metadata-only deployments can reuse a previously published build of this exact release.
if [ -n "${RAVENRS_SITE_ASSETS_FROM:-}" ]; then
    assets="$(cd "$RAVENRS_SITE_ASSETS_FROM" && pwd)"
    case "$site/" in "$assets/"*) echo "output must be outside the reused build" >&2; exit 1;; esac
    version=$(python3 -c 'import pathlib,re; print(re.search(r"\[workspace.package\][\s\S]*?version = \"([^\"]+)\"",pathlib.Path("Cargo.toml").read_text())[1])')
    if ! rg -Fq "$version" "$assets/api/ravenrs/index.html"; then
        echo "the reused API does not match workspace version $version" >&2; exit 1
    fi
    for wasm in pkg/site_home_bg.wasm demo/pkg/component_gallery_bg.wasm; do
        test -f "$assets/$wasm" || { echo "missing published asset: $wasm" >&2; exit 1; }
    done
    rsync -a --exclude=.git --exclude=.vercel --exclude=vercel.json "$assets/" "$site/"
    RAVENRS_API_DOCS_DIR="$site/api" ./scripts/check-component-list.py
else
    ./scripts/build-web.sh hello-world
    ./scripts/build-web.sh quick-app
    ./scripts/build-web.sh quick-plan
    ./scripts/build-web.sh feature-center
    ./scripts/build-web.sh component-gallery
    ./scripts/build-web.sh report-designer
    ./scripts/build-web.sh site-home
    cargo doc --no-deps --workspace --features ravenrs-widgets/code --exclude hello-world --exclude quick-app --exclude quick-plan --exclude feature-center --exclude component-gallery --exclude report-designer --exclude site-home --exclude family-finance --quiet
    ./scripts/check-component-list.py
fi
# Markdown is rendered locally; the same docs ship on Pages and Vercel.
npm ci --prefix site/docs --ignore-scripts --no-audit --no-fund
node scripts/build-docs.mjs "$site/docs"

mkdir -p "$site/demo" "$site/playground" "$site/quick" "$site/hello" "$site/designer" "$site/assets" "$site/components"
# The home page is a RavenRS app: its page and its WebAssembly go at the root.
if [ -z "${RAVENRS_SITE_ASSETS_FROM:-}" ]; then cp -R examples/site-home/web/. "$site/"; fi
cp -R site/components/. "$site/components/"
./scripts/gen-component-pages.py "$site/components"
if [ -z "${RAVENRS_SITE_ASSETS_FROM:-}" ]; then cp -R examples/component-gallery/web/. "$site/demo/"; fi
# The playground of every example: its own page over the gallery's build, which `demo/` holds.
cp -R site/playground/. "$site/playground/"
if [ -z "${RAVENRS_SITE_ASSETS_FROM:-}" ]; then cp -R examples/hello-world/web/. "$site/hello/"; fi
if [ -z "${RAVENRS_SITE_ASSETS_FROM:-}" ]; then cp -R examples/quick-app/web/. "$site/quick/"; fi
if [ -z "${RAVENRS_SITE_ASSETS_FROM:-}" ]; then cp -R examples/report-designer/web/. "$site/designer/"; fi
# What the installed app reads to learn of a new version, when raw.githubusercontent.com does not answer.
mkdir -p "$site/family-finance"
cp examples/family-finance/novidades.json examples/family-finance/historico-de-novidades.json "$site/family-finance/"
cp -R site/assets/. "$site/assets/"
mkdir -p "$site/api"
if [ -z "${RAVENRS_SITE_ASSETS_FROM:-}" ]; then
    cp -R "${CARGO_TARGET_DIR:-target}/doc/." "$site/api/"
fi
node scripts/build-seo.mjs "$site"
node scripts/check-site-seo.mjs "$site"
