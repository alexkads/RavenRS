#!/usr/bin/env bash
# Builds the site locally and publishes the same files to Vercel, as a static site at the root
# of its domain. Vercel builds nothing: the WebAssembly is made on this machine, as for GitHub
# Pages. Needs the Vercel CLI, signed in (`vercel login`).
#
#   scripts/deploy-vercel.sh            a preview, at an address of its own
#   scripts/deploy-vercel.sh --prod     the production address
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
project="${VERCEL_PROJECT:-ravenrs}"

rev="$(git rev-parse --short HEAD)"
if [ -n "$(git status --porcelain)" ]; then
  echo "warning: the working tree has uncommitted changes; they will be published" >&2
fi

site="$(mktemp -d)"
trap 'rm -rf "$site"' EXIT
RAVENRS_SITE_URL="${RAVENRS_SITE_URL:-https://ravenrs.vercel.app}" ./scripts/build-site.sh "$site"
# Vercel keeps `api/` for functions and serves no file in it, so the API docs go in `reference/`
# and `site/vercel.json` sends `/api/...` there: every link stays as it is on GitHub Pages.
mv "$site/api" "$site/reference"
# Folders end in /, so the relative links of each page keep pointing where they should.
# build-site.sh generated vercel.json, including prerendered playground query routes.

cd "$site"
# The project is made the first time; after that this only links to it.
vercel project add "$project" >/dev/null 2>&1 || true
vercel link --yes --project "$project" >/dev/null
# One archive instead of thousands of files: the API docs alone are many.
vercel deploy --yes --archive=tgz --meta "rev=$rev" "$@"
