#!/usr/bin/env bash
# Copy the daemon's OpenAPI spec into the docs site's public/ directory
# so /openapi.yaml resolves at runtime and Redoc can fetch it. Run from
# the docs package via `npm run build` (prebuild hook).
#
# Two execution contexts:
#
#   1. Local dev / CI inside the monorepo: `git rev-parse` succeeds,
#      we copy from `crates/sbo3l-server/openapi.yaml` (canonical).
#   2. Vercel build (apps/docs is the upload root, no .git dir): fall
#      back to a relative walk from this script's directory (apps/docs/
#      scripts/../../..) which lands at the upload root. If `crates/`
#      is part of the upload (monorepo Root Directory unset), we copy.
#      Otherwise honour any pre-existing `public/openapi.yaml` snapshot,
#      or hard-fail so the build doesn't ship a Redoc page pointing
#      at a 404.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEST_DIR="$(cd "$SCRIPT_DIR/.." && pwd)/public"
DEST="${DEST_DIR}/openapi.yaml"

if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  :
else
  REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
fi

SRC="${REPO_ROOT}/crates/sbo3l-server/openapi.yaml"

mkdir -p "$DEST_DIR"

if [ -f "$SRC" ]; then
  cp "$SRC" "$DEST"
  echo "Copied OpenAPI spec → $DEST (from $SRC)"
elif [ -f "$DEST" ]; then
  echo "OpenAPI upstream not reachable from $SRC; using existing $DEST snapshot."
else
  echo "ERROR: cannot find OpenAPI spec at $SRC and no snapshot at $DEST." >&2
  echo "       The /api Redoc page would 404 — refusing to build." >&2
  exit 1
fi
