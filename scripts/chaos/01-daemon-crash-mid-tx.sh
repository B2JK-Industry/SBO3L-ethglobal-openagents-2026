#!/usr/bin/env bash
# Scenario 1 — Daemon crash mid-tx.
#
# What it proves: the daemon's audit chain is recoverable from disk after
# SIGKILL during a request. Partial state must NOT corrupt the chain;
# strict-hash verifier must accept the post-crash chain.
#
# Mechanism:
#   1. Start the daemon.
#   2. POST a request; the response confirms the audit row landed.
#   3. SIGKILL the daemon (no graceful shutdown).
#   4. Capture audit chain "before" snapshot of the on-disk DB.
#   5. Restart the daemon against the same DB.
#   6. POST a follow-up request; verify it linked correctly to the prior
#      `prev_event_hash`.
#   7. Run `sbo3l verify-audit --strict-hash` on the SQLite-exported
#      JSONL → expect rc=0.
#
# This is "best-effort mid-tx" — true mid-transaction kill is racy in
# bash, but the scenario covers the common case: kill after a successful
# request, restart, verify the chain re-mounts cleanly. The strict-hash
# verifier is the load-bearing assertion.

set -euo pipefail
. "$(dirname "$0")/lib-common.sh"
scenario_init "01-crash"

DB="$SCENARIO_DIR/sbo3l.db"
# Pre-test cleanup happens HERE only. The post-SIGKILL `daemon_start`
# below MUST NOT wipe the DB — that would erase the audit chain we
# just wrote and turn this scenario into a tautological "fresh
# daemon's chain has 1 entry after 1 POST" check (the bug the
# round-6 chaos report flagged: count_after=1 instead of 2).
daemon_db_reset "$DB"
daemon_start "$DB"

# Crockford-base32 ULID — no I/L/O/U so the schema's regex
# `^[0-7][0-9A-HJKMNP-TV-Z]{25}$` accepts.
PAYLOAD=$(fixture_aprp "01HCRASH000000000000000Z1A")
RESP=$(http_post "/v1/payment-requests" "$PAYLOAD")
HTTP=$(printf '%s' "$RESP" | tail -n1)
[ "$HTTP" = "200" ] || record_fail "first request HTTP=$HTTP"
audit_dump "$SCENARIO_DIR/before.json"
COUNT_BEFORE=$(jq 'length' "$SCENARIO_DIR/before.json")

# SIGKILL — no graceful shutdown.
kill -KILL "$DAEMON_PID" 2>/dev/null || true
DAEMON_PID=
sleep 0.5

# Restart against the same DB.
daemon_start "$DB"

PAYLOAD=$(fixture_aprp "01HCRASH000000000000000Z2A")
RESP=$(http_post "/v1/payment-requests" "$PAYLOAD")
HTTP=$(printf '%s' "$RESP" | tail -n1)
[ "$HTTP" = "200" ] || record_fail "post-restart request HTTP=$HTTP"
audit_dump "$SCENARIO_DIR/after.json"
COUNT_AFTER=$(jq 'length' "$SCENARIO_DIR/after.json")

# The chain must have grown by exactly one event since the crash.
# Strict equality (== COUNT_BEFORE + 1) catches both the
# original bug (count stays at 1 because pre-restart DB was wiped)
# AND a future failure mode where a request silently writes 2 rows.
if [ "$COUNT_AFTER" -eq $((COUNT_BEFORE + 1)) ]; then
  record_pass "audit chain grew $COUNT_BEFORE → $COUNT_AFTER after restart"
else
  record_fail "audit chain did not advance: $COUNT_BEFORE → $COUNT_AFTER (expected $((COUNT_BEFORE + 1)))"
fi

# Total post-restart count must be exactly 2 — one event per nonce
# submitted, no extras, no losses. Pins the chaos-1 bug.
if [ "$COUNT_AFTER" -eq 2 ]; then
  record_pass "post-restart audit_events count == 2 (one per nonce submitted)"
else
  record_fail "post-restart audit_events count == $COUNT_AFTER (expected 2)"
fi

# Optional: if sbo3l CLI is installed, run strict-hash verifier.
if command -v sbo3l > /dev/null 2>&1; then
  if sbo3l verify-audit --db "$DB" --strict-hash > "$SCENARIO_DIR/verify.log" 2>&1; then
    record_pass "verify-audit --strict-hash rc=0"
  else
    record_fail "verify-audit --strict-hash rc=$? (see verify.log)"
  fi
fi

scenario_finish "01-crash"
exit $?
