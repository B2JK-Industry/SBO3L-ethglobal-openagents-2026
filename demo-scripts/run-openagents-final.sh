#!/usr/bin/env bash
# Mandate ETHGlobal Open Agents — final demo runner.
#
# Status: PARTIAL. The full demo (real-agent harness + sponsor adapters +
# audit chain verification + sponsor execution) lights up as later slices land.
# For now this script runs every gate that is wired and clearly labels what is
# still pending so that judges, CI and the orchestrator share one source of truth.
set -euo pipefail

cd "$(dirname "$0")/.."

bold() { printf '\033[1m%s\033[0m\n' "$1"; }
ok()   { printf '  \033[32mok\033[0m %s\n' "$1"; }
todo() { printf '  \033[33mTODO\033[0m %s\n' "$1"; }

bold "Mandate Open Agents demo (partial)"
echo

bold "1. Build CLI"
cargo build --quiet --bin mandate
ok "cargo build --bin mandate"
echo

bold "2. APRP schema gate"
./target/debug/mandate aprp validate test-corpus/aprp/golden_001_minimal.json >/dev/null
ok "golden_001_minimal.json passes schema"
if ./target/debug/mandate aprp validate test-corpus/aprp/adversarial_unknown_field.json 2>/dev/null; then
  echo "FAIL: adversarial fixture must be rejected"
  exit 1
fi
ok "adversarial_unknown_field.json is rejected with schema.unknown_field"
./target/debug/mandate aprp run-corpus
echo

bold "3. Locked golden APRP request_hash"
EXPECTED=$(tr -d '\n' < test-corpus/aprp/golden_001_minimal.hash)
ACTUAL=$(./target/debug/mandate aprp hash test-corpus/aprp/golden_001_minimal.json)
if [[ "$EXPECTED" != "$ACTUAL" ]]; then
  echo "FAIL: golden APRP request_hash drifted"
  echo "  expected: $EXPECTED"
  echo "  actual:   $ACTUAL"
  exit 1
fi
ok "request_hash = $ACTUAL"
echo

bold "4. Pending slices"
todo "Policy engine + budget evaluation"
todo "SQLite storage + hash-chained audit log"
todo "Payment-request HTTP API"
todo "Research-agent harness (legit-x402, prompt-injection)"
todo "ENS identity adapter"
todo "KeeperHub guarded execution adapter"
todo "Uniswap guarded swap adapter (stretch)"
echo

bold "Demo runner: partial gates passed."
