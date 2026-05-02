#!/usr/bin/env bash
# Scenario 4 — Concurrent identical requests (idempotency race).
#
# What it proves: 50 concurrent POSTs with the same Idempotency-Key
# produce exactly one 200 (or all 200 with byte-identical body if the
# state machine got far enough to cache); the remainder are 409
# `protocol.idempotency_in_flight`. Audit chain has exactly ONE event,
# not 50.
#
# This is the load-bearing concurrency test for #102's state machine.

set -euo pipefail
. "$(dirname "$0")/lib-common.sh"
scenario_init "04-race"

DB="$SCENARIO_DIR/sbo3l.db"
daemon_start "$DB"

audit_dump "$SCENARIO_DIR/before.json"
COUNT_BEFORE=$(jq 'length' "$SCENARIO_DIR/before.json")

KEY="01CHAOS04RACE16CHARSUNIQU"
<<<<<<< HEAD
PAYLOAD=$(fixture_aprp "01HCHAOS04000000000000001")
=======
PAYLOAD=$(fixture_aprp "01HCHA0S040000000000000001")
>>>>>>> 37c25f8 (docs+scripts: round 4 — Trust DNS essay, chaos run artifacts, watcher, Lighthouse, rehearsal runbook)

# Fire 50 concurrent requests with the same idempotency key.
echo "[chaos] firing 50 concurrent same-key POSTs" >> "$SCENARIO_DIR/result.txt"
: > "$SCENARIO_DIR/responses.txt"
for i in $(seq 1 50); do
  curl -sk -m 10 -o /dev/null -w "%{http_code}\n" \
    -H "Content-Type: application/json" \
    -H "Idempotency-Key: $KEY" \
    -d "$PAYLOAD" "http://127.0.0.1:$DAEMON_PORT/v1/payment-requests" \
    >> "$SCENARIO_DIR/responses.txt" &
done
wait

audit_dump "$SCENARIO_DIR/after.json"
COUNT_AFTER=$(jq 'length' "$SCENARIO_DIR/after.json")
GROWTH=$((COUNT_AFTER - COUNT_BEFORE))

# Histogram the responses.
sort "$SCENARIO_DIR/responses.txt" | uniq -c | sort -rn > "$SCENARIO_DIR/histogram.txt"
echo "[chaos] response histogram:" >> "$SCENARIO_DIR/result.txt"
cat "$SCENARIO_DIR/histogram.txt" >> "$SCENARIO_DIR/result.txt"

OK_COUNT=$(awk '$2 == "200"' "$SCENARIO_DIR/histogram.txt" | awk '{print $1}')
CONFLICT_COUNT=$(awk '$2 == "409"' "$SCENARIO_DIR/histogram.txt" | awk '{print $1}')
OK_COUNT=${OK_COUNT:-0}; CONFLICT_COUNT=${CONFLICT_COUNT:-0}
TOTAL=$((OK_COUNT + CONFLICT_COUNT))

# Acceptance: at least one 200, no 5xx, total = 50, audit chain grew by
# exactly 1 (single underlying request actually ran the pipeline).
[ "$OK_COUNT" -ge 1 ] || record_fail "no 200 responses (got $OK_COUNT)"
[ "$TOTAL" -eq 50 ] || record_fail "responses count = $TOTAL, expected 50 (some 5xx?)"
if [ "$GROWTH" -eq 1 ]; then
  record_pass "audit chain grew by exactly 1 event despite 50 concurrent same-key POSTs"
else
  record_fail "audit chain grew by $GROWTH events; expected 1 (state machine race?)"
fi
record_pass "200=$OK_COUNT 409=$CONFLICT_COUNT total=$TOTAL"

scenario_finish "04-race"
exit $?
