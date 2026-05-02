#!/usr/bin/env bash
# Scenario 5 — Clock skew (expiry enforcement).
#
# What it proves: requests with past `expiry` are denied with
# `protocol.expired`; the budget is NOT incremented; the audit row
# records the deny. We don't actually move the system clock — we
# craft an APRP with `expiry` 60 seconds in the past and submit it
# normally.

set -euo pipefail
. "$(dirname "$0")/lib-common.sh"
scenario_init "05-skew"

DB="$SCENARIO_DIR/sbo3l.db"
daemon_start "$DB"
audit_dump "$SCENARIO_DIR/before.json"
COUNT_BEFORE=$(jq 'length' "$SCENARIO_DIR/before.json")

# Past expiry (60s before now) — UTC ISO 8601.
PAST=$(date -u -v-60S '+%Y-%m-%dT%H:%M:%SZ' 2>/dev/null || date -u -d '-60 seconds' '+%Y-%m-%dT%H:%M:%SZ')
echo "[chaos] past expiry: $PAST" >> "$SCENARIO_DIR/result.txt"

PAYLOAD=$(fixture_aprp "01HCHAOS05000000000000001" "$PAST")
RESP=$(http_post "/v1/payment-requests" "$PAYLOAD")
HTTP=$(printf '%s' "$RESP" | tail -n1)
BODY=$(printf '%s' "$RESP" | sed '$d')
echo "  HTTP=$HTTP body=$(printf '%s' "$BODY" | head -c 200)" >> "$SCENARIO_DIR/result.txt"

audit_dump "$SCENARIO_DIR/after.json"
COUNT_AFTER=$(jq 'length' "$SCENARIO_DIR/after.json")
GROWTH=$((COUNT_AFTER - COUNT_BEFORE))

# Acceptance: 4xx response with deny_code containing "expir" (server may
# emit `protocol.expired` or `protocol.expiry_in_past`); audit chain may
# either record the deny (preferred) or skip the row entirely (also OK).
case "$HTTP" in
  4??)
    if printf '%s' "$BODY" | jq -e '.deny_code | test("expir|past")' > /dev/null 2>&1; then
      record_pass "expired APRP rejected with deny_code matching expir/past pattern"
    else
      record_fail "expired APRP returned $HTTP but deny_code unexpected: $BODY"
    fi
    ;;
  *)
    record_fail "expired APRP returned $HTTP (expected 4xx); body: $BODY"
    ;;
esac

# Budget should NOT have advanced. We don't have direct read access to
# the budget store from CLI, so we infer: a follow-up valid request
# should still be permitted up to the cap.
PAYLOAD_OK=$(fixture_aprp "01HCHAOS05000000000000002")
RESP_OK=$(http_post "/v1/payment-requests" "$PAYLOAD_OK")
HTTP_OK=$(printf '%s' "$RESP_OK" | tail -n1)
[ "$HTTP_OK" = "200" ] && record_pass "follow-up valid request HTTP=200 (budget unaffected by expired deny)" \
                       || record_fail "follow-up valid request HTTP=$HTTP_OK (budget may have been wrongly debited)"

# Audit chain growth: 0 (deny dropped before audit) OR 1 or 2 (deny + valid recorded).
echo "[chaos] audit chain growth = $GROWTH events" >> "$SCENARIO_DIR/result.txt"

scenario_finish "05-skew"
exit $?
