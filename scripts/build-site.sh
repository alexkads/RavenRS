#!/usr/bin/env bash
# Builds the whole site into a folder: the home page, the WebAssembly demos, the playground, the
# component pages and the API docs. Every link in it is relative, so the same folder serves at
# the root of a domain (Vercel) or under a path (GitHub Pages, /RavenRS/).
#
#   scripts/build-site.sh <folder>
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
site="${1:?usage: scripts/build-site.sh <folder>}"
mkdir -p "$site"
site="$(cd "$site" && pwd)"

./scripts/build-web.sh hello-world
./scripts/build-web.sh quick-app
./scripts/build-web.sh quick-plan
./scripts/build-web.sh feature-center
./scripts/build-web.sh component-gallery
./scripts/build-web.sh report-designer
./scripts/build-web.sh site-home
cargo doc --no-deps --workspace --exclude hello-world --exclude quick-app --exclude quick-plan --exclude feature-center --exclude component-gallery --exclude report-designer --exclude site-home --exclude family-finance --quiet
./scripts/check-component-list.py

mkdir -p "$site/demo" "$site/playground" "$site/quick" "$site/hello" "$site/designer" "$site/assets" "$site/components"
# The home page is a RavenRS app: its page and its WebAssembly go at the root.
cp -R examples/site-home/web/. "$site/"
cp -R site/components/. "$site/components/"
./scripts/gen-component-pages.py "$site/components"
cp -R examples/component-gallery/web/. "$site/demo/"
# The playground of every example: its own page over the gallery's build, which `demo/` holds.
cp -R site/playground/. "$site/playground/"
cp -R examples/hello-world/web/. "$site/hello/"
cp -R examples/quick-app/web/. "$site/quick/"
cp -R examples/report-designer/web/. "$site/designer/"
# What the installed app reads to learn of a new version, when raw.githubusercontent.com does not answer.
mkdir -p "$site/family-finance"
cp examples/family-finance/novidades.json examples/family-finance/historico-de-novidades.json "$site/family-finance/"
cp -R site/assets/. "$site/assets/"
mkdir -p "$site/api"
cp -R "${CARGO_TARGET_DIR:-target}/doc/." "$site/api/"
