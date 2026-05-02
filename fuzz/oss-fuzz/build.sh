#!/usr/bin/env bash
# OSS-Fuzz build script for SBO3L.
#
# Path: $SRC/sbo3l/fuzz/oss-fuzz/build.sh — invoked by the OSS-Fuzz Docker
# image at build time. Refer to:
#   https://google.github.io/oss-fuzz/getting-started/new-project-guide/rust-lang/

set -euxo pipefail

cd "$SRC/sbo3l/fuzz"

# Build all fuzz targets in OSS-Fuzz mode.
cargo fuzz build -O --debug-assertions

# Copy each binary + matching seed corpus into $OUT.
for target in aprp_parser capsule_deserialize policy_yaml audit_event canonical_json; do
  cp "target/x86_64-unknown-linux-gnu/release/$target" "$OUT/$target"

  # Optional seed corpus (zip per OSS-Fuzz convention).
  if [[ -d "$SRC/sbo3l/test-corpus/$target" ]]; then
    (cd "$SRC/sbo3l/test-corpus/$target" && zip -r "$OUT/${target}_seed_corpus.zip" .)
  fi
done
