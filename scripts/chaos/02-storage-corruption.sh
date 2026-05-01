#!/usr/bin/env bash
# Scenario 2 — Storage corruption (byte-flip on a payload_hash).
#
# What it proves: the strict-hash verifier rejects a chain whose
# `payload_hash` doesn't recompute from the row's payload. The structural
# verifier (linkage-only) accepts the chain (linkage byte unchanged) —
# THIS IS EXPECTED and demonstrates the strict-vs-structural split.
#
# Mechanism:
#   1. Start daemon, POST 3 requests → 3 audit rows.
#   2. Stop daemon.
#   3. Use sqlite3 to flip ONE bit in the middle row's payload_hash.
#   4. Run strict-hash verifier → expect rejection (`event_hash_mismatch`
#      or `payload_hash_mismatch` deny code).
#   5. Run structural verifier → expect rc=0 (linkage byte intact).

set -euo pipefail
. "$(dirname "$0")/lib-common.sh"
scenario_init "02-corruption"

if ! command -v sqlite3 > /dev/null 2>&1; then
  echo "SKIP: sqlite3 not installed" > "$SCENARIO_DIR/result.txt"
  echo "[chaos] sqlite3 not installed; skipping scenario 02"
  exit 0
fi

DB="$SCENARIO_DIR/sbo3l.db"
daemon_start "$DB"

for i in 1 2 3; do
  PAYLOAD=$(fixture_aprp "01HCHAOS0200000000000000$i")
  RESP=$(http_post "/v1/payment-requests" "$PAYLOAD")
  HTTP=$(printf '%s' "$RESP" | tail -n1)
  [ "$HTTP" = "200" ] || record_fail "seed request $i HTTP=$HTTP"
done
audit_dump "$SCENARIO_DIR/before.json"

daemon_stop

# Flip the middle row's payload_hash. Take the existing hex hash, flip
# the first nibble (XOR 0x80), write it back. SQLite unaffected — the
# strict verifier MUST detect the mismatch.
ORIG=$(sqlite3 "$DB" "SELECT payload_hash FROM audit_events WHERE seq=2")
[ -n "$ORIG" ] || record_fail "could not read seq=2 payload_hash"
FIRST_BYTE=${ORIG:0:2}
REST=${ORIG:2}
FLIPPED=$(printf '%02x' $((0x$FIRST_BYTE ^ 0x80)))${REST}
sqlite3 "$DB" "UPDATE audit_events SET payload_hash = '$FLIPPED' WHERE seq=2"
echo "[chaos] flipped seq=2 payload_hash $ORIG → $FLIPPED" >> "$SCENARIO_DIR/result.txt"

audit_dump "$SCENARIO_DIR/after.json"

if command -v sbo3l > /dev/null 2>&1; then
  # Strict-hash should reject.
  if sbo3l verify-audit --db "$DB" --strict-hash > "$SCENARIO_DIR/strict-hash.log" 2>&1; then
    record_fail "strict-hash verifier UNEXPECTEDLY accepted tampered chain"
  else
    record_pass "strict-hash verifier rejected tampered chain (rc=$?)"
  fi

  # Structural (linkage only) should accept — linkage byte not flipped.
  if sbo3l verify-audit --db "$DB" > "$SCENARIO_DIR/structural.log" 2>&1; then
    record_pass "structural verifier accepted (linkage intact, as designed)"
  else
    record_fail "structural verifier rejected (would mean linkage broke; should not happen)"
  fi
else
  echo "[chaos] sbo3l CLI not installed; skipping verifier checks" >> "$SCENARIO_DIR/result.txt"
fi

scenario_finish "02-corruption"
exit $?
