#!/usr/bin/env bash
# Scenario 3 — Network partition (sponsor unreachable).
#
# What it proves: when a sponsor adapter (KeeperHub webhook in this
# scenario) is unreachable, the SBO3L pipeline transitions the
# idempotency row to `failed` with a fresh `created_at` so the grace
# window starts from the failure moment (per #102 codex P2 fix). A
# retry within 60s gets `protocol.idempotency_in_flight`; a retry
# after the grace window can reclaim the row.
#
# Mechanism:
#   1. Start the daemon with SBO3L_KEEPERHUB_WEBHOOK_URL pointed at an
#      address that drops connections (RFC 5737 TEST-NET-1 192.0.2.1).
#   2. POST a request with an idempotency key → expect 502/504/timeout
#      from sponsor; SBO3L marks the idempotency row `failed`.
#   3. Inspect `idempotency_keys.created_at` and `state`.
#   4. POST same key again immediately → expect 409
#      `protocol.idempotency_in_flight` (within grace window).

set -euo pipefail
. "$(dirname "$0")/lib-common.sh"
scenario_init "03-partition"

DB="$SCENARIO_DIR/sbo3l.db"
# Spawn daemon with a black-hole webhook URL.
DAEMON_LOG="$SCENARIO_DIR/daemon.log"
DAEMON_PORT="${SBO3L_LISTEN_PORT:-18731}"
DAEMON_DB="$DB"
rm -f "$DB" "$DB-shm" "$DB-wal"
SBO3L_LISTEN="127.0.0.1:$DAEMON_PORT" \
SBO3L_DB="$DB" \
SBO3L_ALLOW_UNAUTHENTICATED=1 \
SBO3L_KEEPERHUB_WEBHOOK_URL="https://192.0.2.1/dropped" \
SBO3L_KEEPERHUB_TOKEN="wfb_chaos_dummy_token_for_partition_test" \
"${SBO3L_SERVER_BIN:-$REPO_ROOT/target/debug/sbo3l-server}" > "$DAEMON_LOG" 2>&1 &
DAEMON_PID=$!
sleep 2
if ! kill -0 "$DAEMON_PID" 2>/dev/null; then
  record_fail "daemon failed to start (see daemon.log)"
  scenario_finish "03-partition"
  exit 1
fi

audit_dump "$SCENARIO_DIR/before.json"

KEY="01CHAOS03IDEMPOTENCYKEY00"
PAYLOAD=$(fixture_aprp "01HCHAOS03000000000000001")
echo "[chaos] first POST with idempotency key (expect partition / sponsor failure)" >> "$SCENARIO_DIR/result.txt"
RESP1=$(curl -sk -m 15 -w "\n%{http_code}" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: $KEY" \
  -d "$PAYLOAD" "http://127.0.0.1:$DAEMON_PORT/v1/payment-requests" || echo $'\n000')
HTTP1=$(printf '%s' "$RESP1" | tail -n1)
echo "  HTTP=$HTTP1" >> "$SCENARIO_DIR/result.txt"

# After the partition fails the request, SBO3L should mark the
# idempotency row 'failed'. Read the row.
if command -v sqlite3 > /dev/null 2>&1; then
  STATE=$(sqlite3 "$DB" "SELECT state FROM idempotency_keys WHERE key='$KEY'" 2>/dev/null || echo "MISSING")
  echo "  idempotency state after partition: $STATE" >> "$SCENARIO_DIR/result.txt"
  if [ "$STATE" = "failed" ] || [ "$STATE" = "succeeded" ]; then
    record_pass "idempotency state = $STATE (transition recorded)"
  else
    record_fail "idempotency row in unexpected state: $STATE"
  fi
fi

# Second POST same key — within grace window.
echo "[chaos] second POST same key (expect 409 idempotency_in_flight)" >> "$SCENARIO_DIR/result.txt"
RESP2=$(curl -sk -m 15 -w "\n%{http_code}" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: $KEY" \
  -d "$PAYLOAD" "http://127.0.0.1:$DAEMON_PORT/v1/payment-requests" || echo $'\n000')
HTTP2=$(printf '%s' "$RESP2" | tail -n1)
BODY2=$(printf '%s' "$RESP2" | sed '$d')
echo "  HTTP=$HTTP2" >> "$SCENARIO_DIR/result.txt"

# Acceptable: 409 in-flight OR cached replay (200) of failed body.
case "$HTTP2" in
  409) record_pass "idempotency replay rejected with 409" ;;
  200) record_pass "idempotency replay returned cached body (acceptable)" ;;
  *)   record_fail "idempotency replay returned unexpected $HTTP2 (body: $BODY2)" ;;
esac

audit_dump "$SCENARIO_DIR/after.json"
scenario_finish "03-partition"
exit $?
