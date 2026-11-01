#!/usr/bin/env bash
# Builds the landing page and the WebAssembly demo locally and publishes them to
# the gh-pages branch, which GitHub Pages serves at https://alexkads.github.io/RavenRS/.
# No hosted CI is involved: everything is built on this machine.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

remote="$(git remote get-url origin)"
rev="$(git rev-parse --short HEAD)"
if [ -n "$(git status --porcelain)" ]; then
  echo "warning: the working tree has uncommitted changes; they will be published" >&2
fi

./scripts/build-web.sh hello-world
./scripts/build-web.sh component-gallery
./scripts/build-web.sh report-designer
cargo doc --no-deps --workspace --exclude hello-world --exclude component-gallery --exclude report-designer --exclude family-finance --quiet
./scripts/check-component-list.py

site="$(mktemp -d)"
trap 'rm -rf "$site"' EXIT
mkdir -p "$site/demo" "$site/hello" "$site/designer" "$site/assets" "$site/components"
cp site/index.html "$site/"
cp -R site/components/. "$site/components/"
./scripts/gen-component-pages.py "$site/components"
cp -R examples/component-gallery/web/. "$site/demo/"
cp -R examples/hello-world/web/. "$site/hello/"
cp -R examples/report-designer/web/. "$site/designer/"
# What the installed app reads to learn of a new version, when raw.githubusercontent.com does not answer.
mkdir -p "$site/family-finance"
cp examples/family-finance/novidades.json examples/family-finance/historico-de-novidades.json "$site/family-finance/"
cp -R site/assets/. "$site/assets/"
mkdir -p "$site/api"
cp -R target/doc/. "$site/api/"
touch "$site/.nojekyll"

cd "$site"
git init -q -b gh-pages
git add -A
git -c user.name="$(git -C "$root" config user.name)" -c user.email="$(git -C "$root" config user.email)" \
  commit -q -m "Publish site from $rev"
git push -q -f "$remote" gh-pages
echo "Published $rev to gh-pages. GitHub Pages updates in about a minute."
