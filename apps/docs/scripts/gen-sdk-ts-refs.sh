#!/usr/bin/env bash
# Generate TypeScript SDK reference HTML from published @sbo3l/* packages.
#
# Run locally: bash apps/docs/scripts/gen-sdk-ts-refs.sh
# CI: .github/workflows/sdk-refs.yml fires on npm publish events.
#
# Output → apps/docs/public/sdk-ref/typescript/<package>/  (raw typedoc HTML)
# Wrapper Starlight pages at apps/docs/src/content/docs/reference/sdk-typescript/
# link out to these via /sdk-ref/typescript/<package>/.
set -euo pipefail

PACKAGES=(
  "@sbo3l/sdk"
  "@sbo3l/langchain"
  "@sbo3l/autogen"
  "@sbo3l/vercel-ai"
)

REPO_ROOT="$(git rev-parse --show-toplevel)"
WORK="$(mktemp -d)"
OUT="${REPO_ROOT}/apps/docs/public/sdk-ref/typescript"
CONFIG="${REPO_ROOT}/apps/docs/scripts/typedoc.json"

mkdir -p "$OUT"

for pkg in "${PACKAGES[@]}"; do
  echo "== ${pkg} =="
  pkgdir="${WORK}/${pkg//\//_}"
  mkdir -p "$pkgdir"
  cd "$pkgdir"

  # Pull the published package (no scripts to dodge supply-chain risk).
  npm init -y >/dev/null
  npm install --no-fund --no-audit --ignore-scripts "${pkg}@latest"

  # Locate the package's TS entry; typedoc reads .d.ts.
  pkg_path="node_modules/${pkg}"
  entry=$(node -e "const p=require('./${pkg_path}/package.json');process.stdout.write(p.types||p.typings||'index.d.ts')")

  # Generate.
  short="${pkg##*/}"
  npx --yes typedoc@0.26 \
    --options "${CONFIG}" \
    --entryPoints "${pkg_path}/${entry}" \
    --out "${OUT}/${short}"

  cd "$REPO_ROOT"
done

echo "TS SDK refs generated at ${OUT}/"
