#!/usr/bin/env bash
#
# Makes the image of the agent, raven-ci-agente:<versao> and raven-ci-agente:latest, on the machine
# of the docker the server uses, from the RPM of the same version in tools/raven-ci/pacote/rpm/
# (made by pacote/gerar-rpm.sh). The launcher `docker` runs `raven-ci-agente:latest` unless a job
# names another image.
#
#   tools/raven-ci/pacote/docker/gerar-imagem-agente.sh [docker|podman]
set -euo pipefail
MOTOR="${1:-docker}"
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FERRAMENTA="$(cd "$AQUI/../.." && pwd)"
VERSAO=$(sed -n 's/^version = "\(.*\)"/\1/p' "$FERRAMENTA/Cargo.toml" | head -1)
RPM=$(ls -1 "$FERRAMENTA"/pacote/rpm/raven-ci-"$VERSAO"-*.rpm 2>/dev/null | tail -1 || true)
if [ -z "$RPM" ]; then
  echo "não há o RPM da $VERSAO em pacote/rpm/: rode pacote/gerar-rpm.sh antes" >&2
  exit 1
fi
CONTEXTO=$(mktemp -d)
trap 'rm -rf "$CONTEXTO"' EXIT
cp "$AQUI/Dockerfile.agente" "$CONTEXTO/Dockerfile"
cp "$RPM" "$CONTEXTO/"
"$MOTOR" build --pull --label "raven-ci.versao=$VERSAO" \
  -t "raven-ci-agente:$VERSAO" -t raven-ci-agente:latest "$CONTEXTO"
"$MOTOR" run --rm --entrypoint sh raven-ci-agente:latest -c 'raven-ci-agent --versao && git --version && id -un'
