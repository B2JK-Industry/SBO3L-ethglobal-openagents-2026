#!/usr/bin/env bash
# Build the SBO3L Passport WASM verifier for the marketing site (#110).
#
# Output:
#   apps/marketing/public/wasm/sbo3l_core.js
#   apps/marketing/public/wasm/sbo3l_core_bg.wasm
#   apps/marketing/public/wasm/sbo3l_core.d.ts
#   apps/marketing/public/wasm/package.json
#
# The marketing site's `/proof` page imports `sbo3l_core.js` and calls
# `verify_capsule_strict_json(capsuleJsonString)` against a v2
# self-contained capsule fixture; the verifier runs in-browser so the
# proof page is honestly offline-verifiable (no daemon round-trip).
#
# Re-run this script whenever `crates/sbo3l-core/src/wasm.rs` or any
# of its transitive dependencies change. The CI job
# `.github/workflows/ci.yml::wasm-verifier-build` runs the same
# command on every PR and asserts the artefacts exist.

set -euo pipefail
cd "$(dirname "$0")/.."

# wasm-pack is installable via `cargo install wasm-pack` (~30s) or
# `npm install -g wasm-pack`. CI installs from the prebuilt binary
# release for speed.
if ! command -v wasm-pack >/dev/null 2>&1; then
  echo "::error::wasm-pack not found. Install it via:"
  echo "  cargo install wasm-pack"
  echo "  # or: curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh"
  exit 1
fi

# Ensure the wasm32-unknown-unknown target is installed. rustup add is
# idempotent.
rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true

OUT_DIR="apps/marketing/public/wasm"
mkdir -p "$OUT_DIR"

echo "Building sbo3l-core WASM verifier (target=web, out=$OUT_DIR) ..."
wasm-pack build --target web crates/sbo3l-core --out-dir "../../$OUT_DIR"

# Sanity: assert the expected artefacts landed.
for f in sbo3l_core.js sbo3l_core_bg.wasm sbo3l_core.d.ts package.json; do
  if [ ! -f "$OUT_DIR/$f" ]; then
    echo "::error::wasm-pack output missing: $OUT_DIR/$f"
    exit 1
  fi
done

wasm_size=$(wc -c <"$OUT_DIR/sbo3l_core_bg.wasm" | tr -d ' ')
js_size=$(wc -c <"$OUT_DIR/sbo3l_core.js" | tr -d ' ')
echo "OK — verifier built:"
echo "  sbo3l_core_bg.wasm: $wasm_size bytes"
echo "  sbo3l_core.js:      $js_size bytes"
