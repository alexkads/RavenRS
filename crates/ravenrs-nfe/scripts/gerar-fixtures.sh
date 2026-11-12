#!/bin/sh
# Regenerates the certificates of the mTLS test in fixtures/http-test. None has any fiscal value.
# The sandbox certificates are issued at run time; their keys come from gerar-chaves-laboratorio.sh.
# Usage: scripts/gerar-fixtures.sh [path to an OpenSSL 3 binary]
set -eu

openssl_bin="${1:-/opt/homebrew/bin/openssl}"
root="$(cd "$(dirname "$0")/.." && pwd)"
http="$root/fixtures/http-test"
mkdir -p "$http"
days=7300

# Two certificate authorities: the trusted one and one the client must refuse.
"$openssl_bin" req -x509 -newkey rsa:2048 -nodes -days "$days" -sha256 \
  -subj "/CN=RAVENRS TEST CA - SEM VALOR" \
  -addext "basicConstraints=critical,CA:TRUE" -addext "keyUsage=critical,keyCertSign,cRLSign" \
  -keyout "$http/ca-key.pem" -out "$http/ca.pem"
"$openssl_bin" req -x509 -newkey rsa:2048 -nodes -days "$days" -sha256 \
  -subj "/CN=RAVENRS OTHER CA - SEM VALOR" \
  -addext "basicConstraints=critical,CA:TRUE" -addext "keyUsage=critical,keyCertSign,cRLSign" \
  -keyout "$http/other-ca-key.pem" -out "$http/other-ca.pem"

# The local HTTPS server of the mTLS test.
"$openssl_bin" req -newkey rsa:2048 -nodes -subj "/CN=localhost" \
  -keyout "$http/server-key.pem" -out "$http/server.csr"
printf 'basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\nsubjectAltName=DNS:localhost,IP:127.0.0.1\n' > "$http/server.ext"
"$openssl_bin" x509 -req -in "$http/server.csr" -CA "$http/ca.pem" -CAkey "$http/ca-key.pem" \
  -CAcreateserial -days "$days" -sha256 -extfile "$http/server.ext" -out "$http/server.pem"

# The A1-like client certificate, in a modern PFX and in the older cipher many A1 files still use.
"$openssl_bin" req -newkey rsa:2048 -nodes -subj "/CN=RAVENRS CLIENTE TESTE - SEM VALOR" \
  -keyout "$http/client-key.pem" -out "$http/client.csr"
printf 'basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature\nextendedKeyUsage=clientAuth\n' > "$http/client.ext"
"$openssl_bin" x509 -req -in "$http/client.csr" -CA "$http/ca.pem" -CAkey "$http/ca-key.pem" \
  -CAcreateserial -days "$days" -sha256 -extfile "$http/client.ext" -out "$http/client.pem"
"$openssl_bin" pkcs12 -export -inkey "$http/client-key.pem" -in "$http/client.pem" -certfile "$http/ca.pem" \
  -passout pass:teste-sem-valor -out "$http/client.p12"
"$openssl_bin" pkcs12 -export -legacy -inkey "$http/client-key.pem" -in "$http/client.pem" -certfile "$http/ca.pem" \
  -passout pass:teste-sem-valor -out "$http/client-legacy.p12"

rm -f "$http"/*.csr "$http"/*.ext "$http"/*.srl "$http/ca-key.pem" "$http/other-ca-key.pem" "$http/client-key.pem"
