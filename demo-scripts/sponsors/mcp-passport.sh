#!/usr/bin/env bash
# Sponsor demo: SBO3L MCP stdio JSON-RPC — Passport P3.1.
#
# Drives the freshly-built `sbo3l-mcp` server over a stdin/stdout pipe and
# exercises every shipping tool in the catalogue:
#
#   1. tools/list                       — meta, prove the catalogue is wired
#   2. sbo3l.validate_aprp            — schema check against APRP fixture
#   3. sbo3l.decide  (allow path)     — full pipeline → auto_approved
#   4. sbo3l.decide  (deny path)      — prompt-injection → rejected
#   5. sbo3l.run_guarded_execution    — KeeperHub mock executor on allow
#   6. sbo3l.verify_capsule           — re-verify the production-shaped
#                                         passport-allow.json capsule
#   7. sbo3l.audit_lookup             — IP-3 sister tool: event_id + receipt
#                                         → sbo3l.audit_bundle.v1
#
# A transcript of all requests and responses is written to
# `demo-scripts/artifacts/mcp-transcript.json`. Exit 0 iff every step
# returned the expected result.
#
# This demo is **not** in the 13-gate `run-openagents-final.sh` and **not**
# in the production-shaped runner — it's a separate sponsor surface that
# proves the IP-3 alignment story end-to-end.

set -euo pipefail
cd "$(dirname "$0")/../.."

ARTIFACTS=demo-scripts/artifacts
TRANSCRIPT="$ARTIFACTS/mcp-transcript.json"
mkdir -p "$ARTIFACTS"

bold()  { printf '\033[1m%s\033[0m\n' "$1"; }
ok()    { printf '  \033[32mok\033[0m  %s\n' "$1"; }
fail()  { printf '  \033[31mFAIL\033[0m %s\n' "$1"; }

bold "sbo3l-mcp — sponsor demo (Passport P3.1)"
echo

# Pre-flight: required tooling.
for tool in jq cargo; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "missing dependency: $tool" >&2
    exit 1
  }
done

# Build both binaries upfront. The MCP server doesn't need `sbo3l`, but
# step 3+4 seed the DB via `sbo3l policy activate` to match the runtime
# layout the operator console + production-shaped runner already use.
cargo build --quiet --bin sbo3l --bin sbo3l-mcp

# Fresh DB per run so the nonce store is empty.
WORK=$(mktemp -d -t mcp-passport-demo.XXXXXX)
DB="$WORK/sbo3l.sqlite"
# Round 0 path-sandbox fix: tell the MCP server to treat $WORK as its
# allowed root. Without this, the spawned sbo3l-mcp would default to
# the current working directory (the repo root) and reject the
# /var/folders tempdir DB path with capsule.path_escape. Exporting here
# means every subsequent `./target/debug/sbo3l-mcp` invocation in the
# pipeline (phase 1 batch + verify + audit_lookup) inherits the env.
export SBO3L_MCP_ROOT="$WORK"
trap 'rm -rf "$WORK"' EXIT

./target/debug/sbo3l policy activate \
  test-corpus/policy/reference_low_risk.json \
  --db "$DB" >/dev/null
ok "policy activated in $DB"

# Per-test unique nonces (Crockford base32 — no I/L/O/U). Each request body
# uses a different nonce so the nonce-replay store doesn't collapse two
# calls into one cached response.
nonce_replace () {
  local in="$1"; local suffix="$2"
  jq --arg n "01HTAWX5K3R8YV9NQB7C6P${suffix}" '.nonce = $n' "$in"
}

ALLOW_APRP=$(nonce_replace test-corpus/aprp/golden_001_minimal.json "MCP1")
DENY_APRP=$(nonce_replace  test-corpus/aprp/deny_prompt_injection_request.json "MCP2")
GUARD_APRP=$(nonce_replace test-corpus/aprp/golden_001_minimal.json "MCP3")

# Audit + receipt signer pubkeys.
#
# These are the deterministic dev pubkeys baked into
# `sbo3l_server::AppState::new` — derived from
# `DevSigner::from_seed("audit-signer-v1", [11u8; 32]).verifying_key_hex()`
# and `DevSigner::from_seed("decision-signer-v1", [7u8; 32]).verifying_key_hex()`.
# They are PUBLIC (the seeds live in `crates/sbo3l-server/src/lib.rs`
# under a ⚠ DEV ONLY ⚠ banner) and therefore safe to inline here.
#
# The values are pinned by `dev_pubkeys_match_canonical_constants` in
# `crates/sbo3l-mcp/tests/jsonrpc_integration.rs` — if the seeds ever
# change, that test breaks and this demo updates in the same commit.
AUDIT_PUBKEY="66be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a"
RECEIPT_PUBKEY="ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c"

# ----------------------------------------------------------------------------
# Phase 1: requests that don't depend on each other → batch over one stdin.
# ----------------------------------------------------------------------------
PHASE1_REQS="$WORK/phase1.req"
PHASE1_RESPS="$WORK/phase1.resp"

{
  echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'
  jq -nc --argjson aprp "$ALLOW_APRP" \
    '{jsonrpc:"2.0",id:2,method:"sbo3l.validate_aprp",params:{aprp:$aprp}}'
  jq -nc --argjson aprp "$ALLOW_APRP" --arg db "$DB" \
    '{jsonrpc:"2.0",id:3,method:"sbo3l.decide",params:{aprp:$aprp,db:$db}}'
  jq -nc --argjson aprp "$DENY_APRP"  --arg db "$DB" \
    '{jsonrpc:"2.0",id:4,method:"sbo3l.decide",params:{aprp:$aprp,db:$db}}'
  jq -nc --argjson aprp "$GUARD_APRP" --arg db "$DB" \
    '{jsonrpc:"2.0",id:5,method:"sbo3l.run_guarded_execution",params:{aprp:$aprp,db:$db,executor:"keeperhub"}}'
} > "$PHASE1_REQS"

./target/debug/sbo3l-mcp < "$PHASE1_REQS" > "$PHASE1_RESPS" 2>/dev/null

# Sanity-check each response.
LIST_RESP=$(jq    -c 'select(.id==1)' "$PHASE1_RESPS")
VALIDATE_RESP=$(jq -c 'select(.id==2)' "$PHASE1_RESPS")
ALLOW_RESP=$(jq    -c 'select(.id==3)' "$PHASE1_RESPS")
DENY_RESP=$(jq     -c 'select(.id==4)' "$PHASE1_RESPS")
GUARD_RESP=$(jq    -c 'select(.id==5)' "$PHASE1_RESPS")

jq -e '.result | length == 6' <<<"$LIST_RESP" >/dev/null \
  && ok "tools/list returned 6 tools" \
  || { fail "tools/list shape wrong"; exit 1; }

jq -e '.result.ok == true' <<<"$VALIDATE_RESP" >/dev/null \
  && ok "sbo3l.validate_aprp ok (request_hash=$(jq -r '.result.request_hash[0:12]' <<<"$VALIDATE_RESP")…)" \
  || { fail "validate_aprp wrong"; exit 1; }

ALLOW_STATUS=$(jq -r '.result.status' <<<"$ALLOW_RESP")
ALLOW_EVENT_ID=$(jq -r '.result.audit_event_id' <<<"$ALLOW_RESP")
ALLOW_RECEIPT=$(jq    '.result.receipt'      <<<"$ALLOW_RESP")
[ "$ALLOW_STATUS" = "auto_approved" ] \
  && ok "sbo3l.decide allow → auto_approved (audit_event_id=$ALLOW_EVENT_ID)" \
  || { fail "allow path returned status=$ALLOW_STATUS"; exit 1; }

DENY_STATUS=$(jq -r '.result.status'    <<<"$DENY_RESP")
DENY_CODE=$(jq   -r '.result.deny_code' <<<"$DENY_RESP")
[ "$DENY_STATUS" = "rejected" ] \
  && ok "sbo3l.decide deny → rejected (deny_code=$DENY_CODE)" \
  || { fail "deny path returned status=$DENY_STATUS"; exit 1; }

GUARD_STATUS=$(jq -r '.result.execution.status'     <<<"$GUARD_RESP")
GUARD_REF=$(jq    -r '.result.execution.execution_ref' <<<"$GUARD_RESP")
[ "$GUARD_STATUS" = "submitted" ] \
  && ok "sbo3l.run_guarded_execution → keeperhub mock submitted (ref=$GUARD_REF)" \
  || { fail "run_guarded_execution status=$GUARD_STATUS"; exit 1; }

# ----------------------------------------------------------------------------
# Phase 2: verify the production-shaped passport capsule (if it exists)
# ----------------------------------------------------------------------------
CAPSULE="$ARTIFACTS/passport-allow.json"
if [ -f "$CAPSULE" ]; then
  VERIFY_REQ=$(jq -nc --slurpfile c "$CAPSULE" \
    '{jsonrpc:"2.0",id:6,method:"sbo3l.verify_capsule",params:{capsule:$c[0]}}')
  VERIFY_RESP=$(printf '%s\n' "$VERIFY_REQ" | ./target/debug/sbo3l-mcp 2>/dev/null)
  jq -e '.result.ok == true' <<<"$VERIFY_RESP" >/dev/null \
    && ok "sbo3l.verify_capsule on $CAPSULE → ok" \
    || { fail "verify_capsule on $CAPSULE failed"; exit 1; }
else
  VERIFY_RESP='null'
  echo "  -- skipping capsule verify: $CAPSULE not present (run bash demo-scripts/run-production-shaped-mock.sh first)"
fi

# ----------------------------------------------------------------------------
# Phase 3: audit_lookup (IP-3) — needs allow's event_id + receipt
# ----------------------------------------------------------------------------
LOOKUP_REQ=$(jq -nc \
  --arg event_id "$ALLOW_EVENT_ID" \
  --arg db "$DB" \
  --argjson receipt "$ALLOW_RECEIPT" \
  --arg audit_pk "$AUDIT_PUBKEY" \
  --arg receipt_pk "$RECEIPT_PUBKEY" \
  '{jsonrpc:"2.0",id:7,method:"sbo3l.audit_lookup",params:{audit_event_id:$event_id,db:$db,receipt:$receipt,receipt_pubkey:$receipt_pk,audit_pubkey:$audit_pk}}')
LOOKUP_RESP=$(printf '%s\n' "$LOOKUP_REQ" | ./target/debug/sbo3l-mcp 2>/dev/null)

if jq -e '.result.ok == true' <<<"$LOOKUP_RESP" >/dev/null; then
  BUNDLE_TYPE=$(jq -r '.result.bundle.bundle_type'                 <<<"$LOOKUP_RESP")
  BUNDLE_LEN=$( jq    '.result.bundle.audit_chain_segment | length' <<<"$LOOKUP_RESP")
  ok "sbo3l.audit_lookup → $BUNDLE_TYPE (chain_length=$BUNDLE_LEN, audit_event_id=$ALLOW_EVENT_ID)"
else
  fail "audit_lookup failed: $(jq -c '.error' <<<"$LOOKUP_RESP")"
  exit 1
fi

# ----------------------------------------------------------------------------
# Transcript
# ----------------------------------------------------------------------------
jq -nc \
  --argjson list      "$LIST_RESP" \
  --argjson validate  "$VALIDATE_RESP" \
  --argjson allow     "$ALLOW_RESP" \
  --argjson deny      "$DENY_RESP" \
  --argjson guard     "$GUARD_RESP" \
  --argjson verify    "$VERIFY_RESP" \
  --argjson lookup    "$LOOKUP_RESP" \
  '{
    schema: "sbo3l.mcp_demo_transcript.v1",
    generated_at: now | todate,
    requests_in_order: ["tools/list","sbo3l.validate_aprp","sbo3l.decide(allow)","sbo3l.decide(deny)","sbo3l.run_guarded_execution","sbo3l.verify_capsule","sbo3l.audit_lookup"],
    responses: {
      "tools/list":                      $list,
      "sbo3l.validate_aprp":           $validate,
      "sbo3l.decide.allow":            $allow,
      "sbo3l.decide.deny":             $deny,
      "sbo3l.run_guarded_execution":   $guard,
      "sbo3l.verify_capsule":          $verify,
      "sbo3l.audit_lookup":            $lookup
    }
  }' | jq '.' > "$TRANSCRIPT"

echo
ok "transcript → $TRANSCRIPT"
echo
bold "All MCP demo steps completed."
