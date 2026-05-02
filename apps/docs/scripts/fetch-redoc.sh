#!/usr/bin/env bash
# Fetch the Redoc standalone bundle into apps/docs/public/redoc/.
# Runs as part of `npm run build` via the prebuild hook so /api always
# serves the latest pinned Redoc release. Idempotent — skip if already
# downloaded at the pinned version.
#
# Same-origin serving keeps CSP `default-src 'self'` intact for every
# route except /api (which adds 'unsafe-eval' for WASM-like Function()
# usage in the Redoc bundle — see apps/docs/vercel.json).
set -euo pipefail

REDOC_VERSION="2.1.5"
REDOC_URL="https://cdn.redocly.com/redoc/v${REDOC_VERSION}/bundles/redoc.standalone.js"
OUT_DIR="$(dirname "$0")/../public/redoc"
OUT_FILE="${OUT_DIR}/redoc.standalone.js"
VERSION_FILE="${OUT_DIR}/.version"

mkdir -p "$OUT_DIR"

if [ -f "$VERSION_FILE" ] && [ "$(cat "$VERSION_FILE")" = "$REDOC_VERSION" ] && [ -f "$OUT_FILE" ]; then
  echo "Redoc ${REDOC_VERSION} already present — skipping fetch."
  exit 0
fi

echo "Fetching Redoc standalone v${REDOC_VERSION}…"
curl -fsSL "$REDOC_URL" -o "$OUT_FILE"
echo "$REDOC_VERSION" > "$VERSION_FILE"
echo "Saved $OUT_FILE ($(wc -c < "$OUT_FILE") bytes)"
