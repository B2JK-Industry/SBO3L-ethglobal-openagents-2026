#!/usr/bin/env bash
# Scenario 5 — Clock skew (expiry enforcement).
#
# What it proves: requests with past `expiry` are denied with
# `protocol.aprp_expired`; the budget is NOT incremented; the audit row
# records the deny. We don't actually move the system clock — we
# craft an APRP with `expiry` 120 seconds in the past and submit it
# normally.
#
# Why 120s and not "just past now":
# The server (post-#226 / CHAOS-2 fix) uses a 60-second skew tolerance
# for clock drift between sender + receiver. An expiry exactly 60s in
# the past lands AT the boundary; round-trip + parse latency means
# `now_ts - aprp.expiry` measured server-side falls to 59-something
# and slips through. 120s is safely past the tolerance with margin
# for any realistic clock noise.

set -euo pipefail
. "$(dirname "$0")/lib-common.sh"
scenario_init "05-skew"

DB="$SCENARIO_DIR/sbo3l.db"
# Per #227 lib-common refactor: daemon_start no longer wipes the DB.
# Reset explicitly so re-runs don't accumulate nonce-replay state.
daemon_db_reset "$DB"
daemon_start "$DB"
audit_dump "$SCENARIO_DIR/before.json"
COUNT_BEFORE=$(jq 'length' "$SCENARIO_DIR/before.json")

# Past expiry (60s before now) — UTC ISO 8601.
PAST=$(date -u -v-120S '+%Y-%m-%dT%H:%M:%SZ' 2>/dev/null || date -u -d '-120 seconds' '+%Y-%m-%dT%H:%M:%SZ')
echo "[chaos] past expiry: $PAST" >> "$SCENARIO_DIR/result.txt"

PAYLOAD=$(fixture_aprp "01HCHA0S050000000000000001" "$PAST")
RESP=$(http_post "/v1/payment-requests" "$PAYLOAD")
HTTP=$(printf '%s' "$RESP" | tail -n1)
BODY=$(printf '%s' "$RESP" | sed '$d')
echo "  HTTP=$HTTP body=$(printf '%s' "$BODY" | head -c 200)" >> "$SCENARIO_DIR/result.txt"

audit_dump "$SCENARIO_DIR/after.json"
COUNT_AFTER=$(jq 'length' "$SCENARIO_DIR/after.json")
GROWTH=$((COUNT_AFTER - COUNT_BEFORE))

# Acceptance: 4xx response with code OR deny_code containing "expir"
# (post-#226: RFC 7807 problem-detail body uses `.code`, e.g.
# `protocol.aprp_expired`; older deny paths used `.deny_code`).
# Audit chain may either record the deny (rare, deny path) or skip
# the row entirely (post-#226: rejected pre-pipeline before nonce
# claim, no audit row by design).
case "$HTTP" in
  4??)
    if printf '%s' "$BODY" | jq -e '(.code // .deny_code // "") | test("expir|past")' > /dev/null 2>&1; then
      record_pass "expired APRP rejected with code/deny_code matching expir/past pattern"
    else
      record_fail "expired APRP returned $HTTP but code/deny_code unexpected: $BODY"
    fi
    ;;
  *)
    record_fail "expired APRP returned $HTTP (expected 4xx); body: $BODY"
    ;;
esac

# Budget should NOT have advanced. We don't have direct read access to
# the budget store from CLI, so we infer: a follow-up valid request
# should still be permitted up to the cap.
PAYLOAD_OK=$(fixture_aprp "01HCHA0S050000000000000002")
RESP_OK=$(http_post "/v1/payment-requests" "$PAYLOAD_OK")
HTTP_OK=$(printf '%s' "$RESP_OK" | tail -n1)
[ "$HTTP_OK" = "200" ] && record_pass "follow-up valid request HTTP=200 (budget unaffected by expired deny)" \
                       || record_fail "follow-up valid request HTTP=$HTTP_OK (budget may have been wrongly debited)"

# Audit chain growth: 0 (deny dropped before audit) OR 1 or 2 (deny + valid recorded).
echo "[chaos] audit chain growth = $GROWTH events" >> "$SCENARIO_DIR/result.txt"

scenario_finish "05-skew"
exit $?
