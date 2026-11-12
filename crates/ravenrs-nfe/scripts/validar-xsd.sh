#!/bin/sh
# Validates what the crate writes and what the sandbox answers against the official XSD.
# Usage: scripts/validar-xsd.sh <folder with the PL_010 schemas, nfe_v4.00.xsd included>
set -eu

if [ "$#" -ne 1 ] || [ ! -f "$1/nfe_v4.00.xsd" ]; then
  echo "uso: $0 <pasta com nfe_v4.00.xsd e os demais esquemas do PL_010>" >&2
  exit 2
fi

NFE_XSD_DIR="$(cd "$1" && pwd)"
export NFE_XSD_DIR
exec cargo test -p ravenrs-nfe --test externos -- --ignored --nocapture
