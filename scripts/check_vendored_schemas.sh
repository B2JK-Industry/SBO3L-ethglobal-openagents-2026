#!/usr/bin/env bash
# F-11: enforce that vendored schemas + migrations stay byte-for-byte
# in sync with their workspace-root canonical sources.
#
# Workspace-root paths are the authoring location:
#   schemas/*.json
#   migrations/*.sql
#
# Crates that need to bundle these into their published crates.io
# package vendor a copy under `crates/<name>/{schemas,migrations}/`.
# The `include_str!` macros in `crates/sbo3l-core/src/schema.rs` and
# `crates/sbo3l-storage/src/db.rs` reference the vendored copies so
# `cargo publish` can package them.
#
# When you edit a workspace schema, also re-copy it into the vendored
# location (`cp schemas/*.json crates/sbo3l-core/schemas/`,
# `cp migrations/*.sql crates/sbo3l-storage/migrations/`). This script
# is the CI-side guard that catches a forgotten copy: it diffs each
# pair and exits 1 on any mismatch.

set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

drift=0

check_pair() {
  local src="$1"
  local dst="$2"
  if [ ! -f "$src" ]; then
    echo "::error::vendored sync: missing source $src"
    drift=1
    return
  fi
  if [ ! -f "$dst" ]; then
    echo "::error::vendored sync: missing vendored copy $dst (run cp '$src' '$dst' to fix)"
    drift=1
    return
  fi
  if ! cmp -s "$src" "$dst"; then
    echo "::error::vendored sync drift: $src and $dst differ; re-copy $src into $dst"
    diff -u "$src" "$dst" | head -30 || true
    drift=1
  fi
}

echo "Verifying schemas vendored into crates/sbo3l-core/schemas/ ..."
for f in schemas/*.json; do
  name=$(basename "$f")
  check_pair "$f" "crates/sbo3l-core/schemas/$name"
done

echo "Verifying migrations vendored into crates/sbo3l-storage/migrations/ ..."
for f in migrations/*.sql; do
  name=$(basename "$f")
  check_pair "$f" "crates/sbo3l-storage/migrations/$name"
done

if [ "$drift" -ne 0 ]; then
  echo
  echo "::error::vendored schema/migration drift detected. The workspace-root"
  echo "  copies under schemas/ and migrations/ are the canonical source;"
  echo "  the vendored copies under crates/<name>/{schemas,migrations}/ are"
  echo "  what cargo publish ships. Re-run:"
  echo "    cp schemas/*.json crates/sbo3l-core/schemas/"
  echo "    cp migrations/*.sql crates/sbo3l-storage/migrations/"
  echo "  then commit the vendored copies."
  exit 1
fi

echo "OK — vendored schemas + migrations are byte-for-byte in sync."
