#!/usr/bin/env bash
# Build per-version documentation snapshots.
#
# Reads apps/docs/src/data/versions.json. For each entry with a non-
# null `tag`, checks out that tag in a temporary git worktree, runs
# `astro build` with a per-version `--base` override, and copies the
# output into apps/docs/dist/<version-id>/. The default (tag: null,
# id: "latest") is built from the current working tree at the apex
# (apps/docs/dist/).
#
# Output layout served by Vercel:
#   sbo3l-docs.vercel.app/                  → latest (current main)
#   sbo3l-docs.vercel.app/v1.0.0/           → tagged v1.0.0 snapshot
#   sbo3l-docs.vercel.app/v1.0.1/           → tagged v1.0.1 snapshot
#   sbo3l-docs.vercel.app/v1.2.0/           → tagged v1.2.0 snapshot
#
# Run from apps/docs/ via `npm run build:versioned`. CI runs it on
# every merge to main + on every tag push.
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
DOCS_DIR="${REPO_ROOT}/apps/docs"
VERSIONS_JSON="${DOCS_DIR}/src/data/versions.json"
DIST="${DOCS_DIR}/dist"
WORKTREES_ROOT="$(mktemp -d)"

cd "$DOCS_DIR"

# 1. Build "latest" from the current working tree at the apex.
echo "== building latest (apex) =="
ASTRO_BASE_PATH="" npm run build

# 2. Build each tagged snapshot in a temp worktree.
TAGS=$(node -e "
const v = require('${VERSIONS_JSON}');
for (const e of v.versions) if (e.tag) console.log(e.id, e.tag);
")

if [ -z "$TAGS" ]; then
  echo "No tagged versions in versions.json — apex build is the entire site."
  exit 0
fi

while read -r ID TAG; do
  [ -z "$ID" ] && continue
  echo "== building ${ID} from tag ${TAG} =="
  WT="${WORKTREES_ROOT}/${ID}"
  git -C "$REPO_ROOT" worktree add --detach "$WT" "$TAG"
  pushd "$WT/apps/docs" >/dev/null
  ASTRO_BASE_PATH="/${ID}" npm install --no-fund --no-audit --silent
  ASTRO_BASE_PATH="/${ID}" npm run build
  mkdir -p "${DIST}/${ID}"
  cp -r dist/* "${DIST}/${ID}/"
  popd >/dev/null
  git -C "$REPO_ROOT" worktree remove --force "$WT"
done <<< "$TAGS"

echo "Versioned docs built at ${DIST}/"
