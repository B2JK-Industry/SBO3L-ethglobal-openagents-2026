#!/usr/bin/env bash
# Copy the daemon's OpenAPI spec into the docs site's public/ directory
# so /openapi.yaml resolves at runtime and Redoc can fetch it. Run from
# the docs package via `npm run build` (prebuild hook).
set -euo pipefail
REPO_ROOT="$(git rev-parse --show-toplevel)"
SRC="${REPO_ROOT}/crates/sbo3l-server/openapi.yaml"
DEST="${REPO_ROOT}/apps/docs/public/openapi.yaml"
test -f "$SRC" || { echo "missing $SRC"; exit 1; }
cp "$SRC" "$DEST"
echo "Copied OpenAPI spec → $DEST"
