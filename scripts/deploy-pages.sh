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
cargo doc --no-deps --workspace --exclude hello-world --exclude component-gallery --quiet

site="$(mktemp -d)"
trap 'rm -rf "$site"' EXIT
mkdir -p "$site/demo" "$site/hello"
cp site/index.html "$site/"
cp -R examples/component-gallery/web/. "$site/demo/"
cp -R examples/hello-world/web/. "$site/hello/"
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
