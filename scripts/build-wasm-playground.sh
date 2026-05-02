#!/usr/bin/env bash
# Build the SBO3L browser playground WASM bundle (R17 P1).
#
# Output:
#   apps/marketing/public/wasm/sbo3l_playground.js
#   apps/marketing/public/wasm/sbo3l_playground_bg.wasm
#   apps/marketing/public/wasm/sbo3l_playground.d.ts
#   apps/marketing/public/wasm/package.json
#
# The marketing site's `/playground` page imports
# `sbo3l_playground.js` and calls:
#   - `decide_aprp_wasm(aprpJson, policyJson)` — real policy engine
#   - `build_capsule_wasm(aprpJson, decisionJson, policyJson, seedHex, issuedAtRfc3339)`
#     → fully self-contained sbo3l.passport_capsule.v2 (passes the
#     6-check strict verifier with no aux input)
#
# Re-run this script whenever `crates/sbo3l-playground/src/*.rs` or
# any of its transitive deps (sbo3l-core, sbo3l-policy) change.

set -euo pipefail
cd "$(dirname "$0")/.."

if ! command -v wasm-pack >/dev/null 2>&1; then
  echo "::error::wasm-pack not found. Install via:"
  echo "  cargo install wasm-pack"
  echo "  # or: curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh"
  exit 1
fi

rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true

OUT_DIR="apps/marketing/public/wasm"
mkdir -p "$OUT_DIR"

echo "Building sbo3l-playground WASM bundle (target=web, out=$OUT_DIR) ..."
wasm-pack build --target web crates/sbo3l-playground --out-dir "../../$OUT_DIR"

for f in sbo3l_playground.js sbo3l_playground_bg.wasm sbo3l_playground.d.ts package.json; do
  if [ ! -f "$OUT_DIR/$f" ]; then
    echo "::error::wasm-pack output missing: $OUT_DIR/$f"
    exit 1
  fi
done

# Honest size report — R17 brief targets ≤250KB gzipped; current
# realistic ceiling is ~1MB gzipped because policy + jsonschema +
# serde_yaml + frost-ed25519 land in the bundle. Surface the number
# rather than fudge it.
WASM_RAW_BYTES=$(wc -c <"$OUT_DIR/sbo3l_playground_bg.wasm")
WASM_GZ_BYTES=$(gzip -9 <"$OUT_DIR/sbo3l_playground_bg.wasm" | wc -c)
echo
echo "✓ sbo3l_playground bundle:"
printf "  raw:   %'d bytes (%.1f MB)\n" "$WASM_RAW_BYTES" "$(echo "scale=1; $WASM_RAW_BYTES/1048576" | bc)"
printf "  gzip:  %'d bytes (%.0f KB)\n" "$WASM_GZ_BYTES" "$(echo "scale=0; $WASM_GZ_BYTES/1024" | bc)"
