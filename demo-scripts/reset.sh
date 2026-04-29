#!/usr/bin/env bash
# Reset hackathon-local demo state. Useful before recording a video or
# re-running the orchestrator from a clean slate.
#
# What is removed:
#   - target/ build artefacts? NO — these are expensive to rebuild and the
#     demo is fully deterministic on top of them.
#   - .sbo3l-state/                  (per-run state if any future slice writes here)
#   - test-corpus/policy/.runtime/     (any runtime cache files)
set -euo pipefail
cd "$(dirname "$0")/.."

bold() { printf '\033[1m%s\033[0m\n' "$1"; }
bold "Reset SBO3L demo state"

removed=0
for path in .sbo3l-state test-corpus/policy/.runtime; do
  if [[ -e "$path" ]]; then
    rm -rf -- "$path"
    echo "  removed $path"
    removed=$((removed + 1))
  fi
done

if [[ $removed -eq 0 ]]; then
  echo "  nothing to reset"
fi
