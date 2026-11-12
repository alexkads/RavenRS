#!/usr/bin/env bash
# Builds the site locally and publishes it to the gh-pages branch, which GitHub Pages serves at
# https://alexkads.github.io/RavenRS/. No hosted CI is involved: everything is built on this
# machine.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

remote="$(git remote get-url origin)"
rev="$(git rev-parse --short HEAD)"
if [ -n "$(git status --porcelain)" ]; then
  echo "warning: the working tree has uncommitted changes; they will be published" >&2
fi

site="$(mktemp -d)"
trap 'rm -rf "$site"' EXIT
./scripts/build-site.sh "$site"
touch "$site/.nojekyll"

cd "$site"
git init -q -b gh-pages
git add -A
git -c user.name="$(git -C "$root" config user.name)" -c user.email="$(git -C "$root" config user.email)" \
  commit -q -m "Publish site from $rev"
git push -q -f "$remote" gh-pages
echo "Published $rev to gh-pages. GitHub Pages updates in about a minute."
