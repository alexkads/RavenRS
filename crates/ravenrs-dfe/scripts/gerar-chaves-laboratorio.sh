#!/bin/sh
# Regenerates the RSA keys the sandbox certificate generator uses, and the outside certificate
# the sandbox must refuse. All of it is public on purpose and has no value: certificates are
# issued from these keys at run time, by `ravenrs_dfe::certificados`.
# Keys are bundled because generating RSA takes seconds in a debug build, on every test.
# Usage: scripts/gerar-chaves-laboratorio.sh [path to an OpenSSL 3 binary]
set -eu

openssl_bin="${1:-/opt/homebrew/bin/openssl}"
root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/fixtures/laboratorio"
mkdir -p "$out"

for nome in ac outra-ac titular; do
  "$openssl_bin" genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out "$out/$nome-key.pem"
done
"$openssl_bin" genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:1024 -out "$out/fraca-1024-key.pem"

# A certificate from outside the laboratory whose issuer names ICP-Brasil, only so the sandbox
# can be tested refusing it. It is self-issued and worth nothing.
"$openssl_bin" req -x509 -newkey rsa:2048 -nodes -days 7300 -sha256 \
  -subj "/C=BR/O=ICP-Brasil/CN=CERTIFICADO DE TESTE EXTERNO - SEM VALOR" \
  -keyout "$out/externo-key.pem" -out "$out/externo-cert.pem"
