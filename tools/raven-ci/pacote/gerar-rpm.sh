#!/usr/bin/env bash
#
# Makes the RPM of Raven CI. Runs ON a Fedora with rpm-build and Rust, from a copy of the
# repository:
#
#   tools/raven-ci/pacote/gerar-rpm.sh PAINEL.tar [LANCAMENTO]
#
# PAINEL.tar is the panel for the browser, made where wasm-bindgen is
# (`scripts/build-web.sh raven-ci`, then `COPYFILE_DISABLE=1 tar --no-xattrs -cf painel.tar -C tools/raven-ci/web index.html pkg` on a Mac).
# The release binaries are built waiting for the lock of the machine, with 2 jobs and nice, under
# a limit of memory, so a build never takes the machine down. The .rpm goes to
# tools/raven-ci/pacote/rpm/.
set -euo pipefail
PAINEL="$(realpath "${1:?diga o painel.tar}")"
LANCAMENTO="${2:-1}"
RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
FERRAMENTA="$RAIZ/tools/raven-ci"
ALVO="${CARGO_TARGET_DIR:-$HOME/oficina/target/raven-ci}"
TRAVA="${TRAVA:-$HOME/oficina/.trava-compilacao}"
VERSAO=$(sed -n 's/^version = "\(.*\)"/\1/p' "$FERRAMENTA/Cargo.toml" | head -1)

echo "compilando a $VERSAO (espera a trava, se outra compilação estiver rodando)…"
# shellcheck disable=SC1091
source "$HOME/.cargo/env"
mkdir -p "$(dirname "$TRAVA")"
(
  exec 9>"$TRAVA"
  flock 9
  cd "$RAIZ"
  compilar=(env CARGO_TARGET_DIR="$ALVO" CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0
    nice -n 10 cargo build --release -q -p raven-ci -p raven-ci-agent -p raven-ci-explicador)
  # Inside a unit of systemd (INVOCATION_ID), the unit already has its limit of memory.
  if [ -n "${INVOCATION_ID:-}" ]; then
    "${compilar[@]}" 9>&-
  else
    systemd-run --user --scope --quiet -p MemoryMax=9G -p MemorySwapMax=2G "${compilar[@]}" 9>&-
  fi
)

TOPO=$(mktemp -d)
trap 'rm -rf "$TOPO"' EXIT
mkdir -p "$TOPO/SOURCES"
cp "$ALVO/release/raven-ci" "$ALVO/release/raven-ci-agent" "$ALVO/release/raven-ci-explicador" \
   "$FERRAMENTA/pacote/raven-ci.service" "$FERRAMENTA/pacote/raven-ci.sysusers" \
   "$FERRAMENTA/pacote/agentes-de-outro-usuario.conf" "$FERRAMENTA/README.md" \
   "$FERRAMENTA/pacote/raven-ci-explicador.socket" "$FERRAMENTA/pacote/raven-ci-explicador@.service" "$TOPO/SOURCES/"
cp "$PAINEL" "$TOPO/SOURCES/painel.tar"

echo "empacotando…"
rpmbuild -bb --quiet \
  --define "_topdir $TOPO" \
  --define "versao $VERSAO" \
  --define "lancamento $LANCAMENTO" \
  "$FERRAMENTA/pacote/raven-ci.spec"

mkdir -p "$FERRAMENTA/pacote/rpm"
cp "$TOPO"/RPMS/x86_64/raven-ci-*"$VERSAO"-*.rpm "$FERRAMENTA/pacote/rpm/"
ls -1 "$FERRAMENTA"/pacote/rpm/raven-ci-*"$VERSAO"-*.rpm
