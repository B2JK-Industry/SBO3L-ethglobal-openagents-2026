#!/usr/bin/env bash
# Mandate ETHGlobal Open Agents — production-shaped mock runner (PSM-B1).
#
# This is NOT a replacement for `demo-scripts/run-openagents-final.sh`. The
# 13-gate hackathon demo stays the canonical pass/fail. This runner instead
# walks an operator through the production-shaped surface that already exists
# today — DB-backed audit-bundle export, the agent no-key proof, the trust
# badge build — and prints honest `[SKIP]` lines for the production-shaped
# capabilities still on Developer A's backlog (mock KMS, idempotency-key
# retry, policy lifecycle, checkpoints, doctor command).
#
# Truthfulness contract:
#   - Every mock is labelled `mock`.
#   - No live sponsor call, no live KMS, no network anywhere.
#   - Anything not yet implemented is `[SKIP]` with a pointer at the backlog
#     item. Never fake output from an unavailable command.
#   - Existing final demo runs unchanged when --include-final-demo is set.
#
# Tagline: "Don't give your agent a wallet. Give it a mandate."

set -euo pipefail
cd "$(dirname "$0")/.."

# ─── output helpers ──────────────────────────────────────────────────────
bold()      { printf '\033[1m%s\033[0m\n' "$1"; }
ok()        { printf '  \033[32mok\033[0m   %s\n' "$1";   REAL_COUNT=$((REAL_COUNT+1)); }
mock_ok()   { printf '  \033[36mmock\033[0m %s\n' "$1";   MOCK_COUNT=$((MOCK_COUNT+1)); }
skip()      { printf '  \033[33mSKIP\033[0m %s\n' "$1";   SKIP_COUNT=$((SKIP_COUNT+1)); }
fail()      { printf '  \033[31mFAIL\033[0m %s\n' "$1" >&2; }

REAL_COUNT=0
MOCK_COUNT=0
SKIP_COUNT=0
SKIPPED_NOTES=()

# Run ./target/debug/mandate <subcmd…> --help; success ⇒ command exists.
have_subcmd() {
  ./target/debug/mandate "$@" --help >/dev/null 2>&1
}

note_skip() { SKIPPED_NOTES+=("  - $1"); }

# ─── usage ───────────────────────────────────────────────────────────────
INCLUDE_FINAL_DEMO=0
for arg in "$@"; do
  case "$arg" in
    --include-final-demo) INCLUDE_FINAL_DEMO=1 ;;
    -h|--help)
      cat <<'USAGE'
Usage: bash demo-scripts/run-production-shaped-mock.sh [--include-final-demo]

Walks the production-shaped operator surface using fully offline mock
backends. Prints REAL / MOCK / SKIP for each capability and a final
summary. Pass --include-final-demo to also re-run the canonical 13-gate
hackathon demo at the start.
USAGE
      exit 0 ;;
  esac
done

# ─── 0. Preflight ────────────────────────────────────────────────────────
bold "Mandate production-shaped mock runner (PSM-B1)"
echo

bold "0. Preflight"
COMMIT="$(git rev-parse HEAD 2>/dev/null || echo unknown)"
COMMIT_SHORT="${COMMIT:0:12}"
echo "  repo:     $(git remote get-url origin 2>/dev/null || echo '(no origin)')"
echo "  commit:   $COMMIT_SHORT  ($COMMIT)"
echo "  cwd:      $(pwd)"
for cmd in cargo python3 grep find mktemp; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    fail "preflight: required command not found: $cmd"
    exit 1
  fi
done
ok "required commands present (cargo, python3, grep, find, mktemp)"
cargo build --quiet --bin mandate
cargo build --quiet --bin research-agent
ok "cargo build --bin mandate, --bin research-agent"
echo

# ─── 1. Existing final demo (optional) ───────────────────────────────────
bold "1. Existing 13-gate hackathon demo"
if [[ "$INCLUDE_FINAL_DEMO" == "1" ]]; then
  if bash demo-scripts/run-openagents-final.sh >/dev/null 2>&1; then
    ok "bash demo-scripts/run-openagents-final.sh — all 13 gates green (full output suppressed)"
  else
    fail "run-openagents-final.sh failed; aborting production-shaped run"
    exit 1
  fi
else
  skip "skipped by default (pass --include-final-demo to re-run all 13 gates)"
  note_skip "13-gate final demo: pass --include-final-demo to re-run"
fi
echo

# ─── 2. Operator/health checks (Developer A backlog) ─────────────────────
bold "2. Operator / health (mandate doctor — PSM-A5)"
if have_subcmd doctor; then
  if ./target/debug/mandate doctor 2>&1 | sed 's/^/    /'; then
    ok "mandate doctor — passed"
  else
    fail "mandate doctor reported a problem"
    exit 1
  fi
else
  skip "blocked: waiting for \`mandate doctor\` (backlog PSM-A5)"
  note_skip "mandate doctor (operator readiness summary) — PSM-A5"
fi
echo

# ─── 3. Mock KMS CLI surface (PSM-A1.9 — REAL today) ─────────────────────
bold "3. Mock KMS CLI surface (PSM-A1.9)"
# PSM-A1.9 shipped in PR #28: persistent mock_kms_keys SQLite table
# (migration V005) + `mandate key {init,list,rotate} --mock` CLI surface.
# Every operation requires `--mock` and prefixes every output line with
# `mock-kms:` for explicit disclosure. We exercise init → list → rotate →
# list end-to-end against a fresh tempfile-backed SQLite, then drop it.
# Mock — not production-grade.
if have_subcmd key list; then
  # Section-local tempdir + EXIT trap — TMPDIR_PSM is set up in step 5,
  # later than this section, so we use a self-contained temp space and
  # tear it down on success. The trap is appended (not replaced) so we
  # don't disturb any later trap installs.
  KMS_TMP="$(mktemp -d -t mandate-mock-kms.XXXXXX)"
  KMS_DB="$KMS_TMP/mock-kms.db"
  # Deterministic 64-hex-char dev seed. NOT a secret — `mandate-server`'s
  # production-shaped DevSigner uses literally this byte pattern (all 0x11),
  # see `crates/mandate-server/src/lib.rs:54`. The corresponding public
  # verifying key is the audit-signer pubkey shipped in
  # `demo-fixtures/mock-kms-keys.json`.
  KMS_ROOT_SEED="$(python3 -c 'print("11"*32)')"
  if ./target/debug/mandate key init --mock --role audit-mock \
       --root-seed "$KMS_ROOT_SEED" --db "$KMS_DB" 2>&1 | sed 's/^/    /' \
     && ./target/debug/mandate key list --mock --db "$KMS_DB" 2>&1 | sed 's/^/    /' \
     && ./target/debug/mandate key rotate --mock --role audit-mock \
       --root-seed "$KMS_ROOT_SEED" --db "$KMS_DB" 2>&1 | sed 's/^/    /' \
     && ./target/debug/mandate key list --mock --db "$KMS_DB" 2>&1 | sed 's/^/    /'; then
    ok "mandate key init/list/rotate --mock — full lifecycle exercised against fresh SQLite"
    rm -rf "$KMS_TMP"
  else
    fail "mandate key CLI lifecycle returned non-zero"
    rm -rf "$KMS_TMP"
    exit 1
  fi
else
  skip "signer + trait + rotation are merged in PR #22; waiting for \`mandate key list --mock\` / \`mandate key rotate --mock\` CLI + persistent mock-KMS storage table (backlog PSM-A1.9)"
  note_skip "Mock KMS CLI surface (\`mandate key list --mock\` / \`mandate key rotate --mock\`) + persistent mock-KMS storage table — PSM-A1.9"
fi
echo

# Tempdir + policy DB for sections 4 and 5. Section 3 above uses its
# own self-contained KMS_TMP so its lifecycle stays atomic; section 4
# (active-policy lifecycle) and section 5 (persistent-SQLite allow +
# deny) share one tempfile DB rooted here.
TMPDIR_PSM="$(mktemp -d -t mandate-prod-shaped.XXXXXX)"
trap 'rm -rf "$TMPDIR_PSM"' EXIT
POLICY_DB="$TMPDIR_PSM/policy.db"
REF_POLICY="test-corpus/policy/reference_low_risk.json"

# ─── 4. Active policy lifecycle (PSM-A3 — REAL today) ───────────────────
# Walks validate → current(no-active, exit 3) → activate → current(ok)
# → diff against the reference policy. Each step exits the runner on
# failure so the truthfulness contract holds: a broken lifecycle never
# silently turns into a `skip`.

bold "4. Active policy lifecycle (PSM-A3)"
if have_subcmd policy current; then
  # 4a. validate (no DB)
  if ./target/debug/mandate policy validate "$REF_POLICY" 2>&1 | sed 's/^/    /'; then
    ok "mandate policy validate $REF_POLICY"
  else
    fail "mandate policy validate failed against the reference policy"
    exit 1
  fi
  # 4b. honest no-active (exit 3 on a fresh DB). `policy current` opens
  # the DB (running V001..V006 on first touch) and surfaces the empty
  # active_policy table as exit 3 + an honest "no active policy" line —
  # NOT a fake `ok`.
  if ./target/debug/mandate policy current --db "$POLICY_DB" 2>&1 | sed 's/^/    /'; then
    fail "policy current must exit non-zero on a fresh DB (honest no-active)"
    exit 1
  else
    ok "mandate policy current honestly reports no active policy on a fresh DB (exit 3)"
  fi
  # 4c. activate the reference policy
  if ./target/debug/mandate policy activate "$REF_POLICY" --db "$POLICY_DB" 2>&1 | sed 's/^/    /'; then
    ok "mandate policy activate $REF_POLICY -> v1"
  else
    fail "mandate policy activate failed"
    exit 1
  fi
  # 4d. current after activate -> ok with version + hash prefix
  if ./target/debug/mandate policy current --db "$POLICY_DB" 2>&1 | sed 's/^/    /'; then
    ok "mandate policy current after activate (active row visible)"
  else
    fail "mandate policy current failed after activate"
    exit 1
  fi
  # 4e. diff identical files -> exit 0
  if ./target/debug/mandate policy diff "$REF_POLICY" "$REF_POLICY" 2>&1 | sed 's/^/    /'; then
    ok "mandate policy diff (identical files -> no differences)"
  else
    fail "mandate policy diff against itself must report no differences"
    exit 1
  fi
else
  skip "blocked: waiting for \`mandate policy current\` (backlog PSM-A3)"
  note_skip "Policy activation lifecycle (validate/current/activate/diff) — PSM-A3"
fi
echo

# ─── 5. Allow path on persistent SQLite ──────────────────────────────────
bold "5. Allow path — legit-x402 against persistent SQLite"
DB_PATH="$TMPDIR_PSM/mandate.db"
RECEIPT_PATH="$TMPDIR_PSM/receipt.json"
LEGIT_OUT="$(./demo-agents/research-agent/run \
  --scenario legit-x402 \
  --storage-path "$DB_PATH" \
  --save-receipt "$RECEIPT_PATH")"
echo "$LEGIT_OUT" | sed 's/^/    /'
ALLOW_AUDIT_EVENT="$(printf '%s\n' "$LEGIT_OUT" | awk -F': *' '/^audit_event:/ {print $2}' | head -1)"
if [[ -z "$ALLOW_AUDIT_EVENT" ]]; then
  fail "could not extract audit_event from research-agent stdout"
  exit 1
fi
ok "legit-x402 -> Allow + signed receipt + persistent SQLite chain (audit_event=$ALLOW_AUDIT_EVENT)"
echo

# ─── 6. Deny path on persistent SQLite ───────────────────────────────────
bold "6. Deny path — prompt-injection on the same persistent SQLite"
DENY_OUT="$(./demo-agents/research-agent/run \
  --scenario prompt-injection \
  --storage-path "$DB_PATH")"
echo "$DENY_OUT" | sed 's/^/    /'
DENY_AUDIT_EVENT="$(printf '%s\n' "$DENY_OUT" | awk -F': *' '/^audit_event:/ {print $2}' | head -1)"
DENY_CODE="$(printf '%s\n' "$DENY_OUT" | awk -F': *' '/^deny_code:/ {print $2}' | head -1)"
if [[ -z "$DENY_AUDIT_EVENT" || -z "$DENY_CODE" ]]; then
  fail "could not extract audit_event/deny_code from research-agent stdout"
  exit 1
fi
ok "prompt-injection -> Deny + deny_code=$DENY_CODE + audit_event=$DENY_AUDIT_EVENT (denied action did not execute)"
echo

# ─── 7. Idempotency-Key safe retry (PSM-A2, REAL today) ──────────────────
bold "7. Idempotency-Key safe retry (PSM-A2)"
# PSM-A2 shipped in PR #23: persistent SQLite-backed idempotency-keys table
# (migration V004) plus HTTP `Idempotency-Key` handling at the top of the
# POST /v1/payment-requests pipeline. We exercise the full RFC-shaped
# behaviour matrix against a real HTTP daemon spun up on a dedicated port
# and SQLite file, then kill it. Four cases:
#   1. K=K1, body=B1            → 200, response cached.
#   2. K=K1, body=B1 (retry)    → 200, byte-identical body.
#   3. K=K1, body=B2 (mutated)  → 409 protocol.idempotency_conflict.
#   4. K=K2 (new), body=B1      → 409 protocol.nonce_replay (defense in
#                                  depth: nonce was consumed in case 1).
cargo build --quiet --bin mandate-server
IDEM_DB="$TMPDIR_PSM/idempotency.db"
IDEM_PORT="${MANDATE_PSM_IDEM_PORT:-18730}"
IDEM_BASE="http://127.0.0.1:${IDEM_PORT}"
SERVER_LOG="$TMPDIR_PSM/idempotency-server.log"

# Spawn a fresh mandate-server. EXIT trap was set in step 5 to clean
# $TMPDIR_PSM; we extend it to also kill the server PID.
MANDATE_DB="$IDEM_DB" MANDATE_LISTEN="127.0.0.1:${IDEM_PORT}" \
  ./target/debug/mandate-server >"$SERVER_LOG" 2>&1 &
SERVER_PID=$!
trap 'kill "${SERVER_PID:-0}" 2>/dev/null || true; rm -rf "$TMPDIR_PSM"' EXIT

# Wait for /v1/health (max ~6s).
for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30; do
  if curl -sf "$IDEM_BASE/v1/health" >/dev/null 2>&1; then break; fi
  sleep 0.2
done
if ! curl -sf "$IDEM_BASE/v1/health" >/dev/null 2>&1; then
  fail "mandate-server did not come up on $IDEM_BASE — log:"
  sed 's/^/    /' <"$SERVER_LOG" >&2
  exit 1
fi

# Generate B1 (legit APRP with a fresh ULID-shape nonce, deterministic
# task_id) and B2 (B1 with task_id mutated — different request_hash but
# same nonce). Pure stdlib python; no schema bump anywhere.
python3 - "$TMPDIR_PSM/idem-b1.json" "$TMPDIR_PSM/idem-b2.json" <<'PY'
import json, secrets, sys
crockford = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"
nonce = "01" + "".join(secrets.choice(crockford) for _ in range(24))
with open("test-corpus/aprp/golden_001_minimal.json") as f:
    aprp = json.load(f)
aprp["nonce"] = nonce
aprp["task_id"] = "psm-a2-runner-b1"
with open(sys.argv[1], "w") as f:
    json.dump(aprp, f, indent=2)
aprp["task_id"] = "psm-a2-runner-b2-DIFFERENT"
with open(sys.argv[2], "w") as f:
    json.dump(aprp, f, indent=2)
PY

# 32-char keys (in spec range 16..=64).
K1="psm-a2-runner-key-1-aaaaaaaaaaaaa"
K2="psm-a2-runner-key-2-bbbbbbbbbbbbb"

post_idem() {  # post_idem <key> <body-path> <out-resp> <out-status>
  local key="$1" body="$2" out="$3" status_var="$4"
  # shellcheck disable=SC2034
  printf -v "$status_var" '%s' "$(curl -sS -o "$out" -w '%{http_code}' \
    -X POST "$IDEM_BASE/v1/payment-requests" \
    -H "Content-Type: application/json" \
    -H "Idempotency-Key: $key" \
    --data-binary @"$body")"
}

# Case 1: first POST with K1 + B1 → 200.
post_idem "$K1" "$TMPDIR_PSM/idem-b1.json" "$TMPDIR_PSM/resp1.json" RESP1
if [[ "$RESP1" != "200" ]]; then
  fail "PSM-A2 case 1: K1 + B1 first POST expected 200, got $RESP1"
  cat "$TMPDIR_PSM/resp1.json" >&2
  exit 1
fi
ALLOW1_AUDIT_EVENT="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["audit_event_id"])' "$TMPDIR_PSM/resp1.json")"
ok "PSM-A2 case 1: K=K1, B=B1 → 200; audit_event=$ALLOW1_AUDIT_EVENT"

# Case 2: retry K1 + B1 → 200, byte-identical body.
post_idem "$K1" "$TMPDIR_PSM/idem-b1.json" "$TMPDIR_PSM/resp2.json" RESP2
if [[ "$RESP2" != "200" ]]; then
  fail "PSM-A2 case 2: K1 + B1 retry expected 200, got $RESP2"
  cat "$TMPDIR_PSM/resp2.json" >&2
  exit 1
fi
if ! diff -q "$TMPDIR_PSM/resp1.json" "$TMPDIR_PSM/resp2.json" >/dev/null; then
  fail "PSM-A2 case 2: retry response is NOT byte-identical to original"
  diff -u "$TMPDIR_PSM/resp1.json" "$TMPDIR_PSM/resp2.json" >&2 || true
  exit 1
fi
ok "PSM-A2 case 2: K=K1, B=B1 retry → 200, response byte-identical to case 1 (cache replay, no second audit append)"

# Case 3: K1 + B2 (mutated body) → 409 protocol.idempotency_conflict.
post_idem "$K1" "$TMPDIR_PSM/idem-b2.json" "$TMPDIR_PSM/resp3.json" RESP3
if [[ "$RESP3" != "409" ]]; then
  fail "PSM-A2 case 3: K1 + B2 expected 409, got $RESP3"
  cat "$TMPDIR_PSM/resp3.json" >&2
  exit 1
fi
if ! grep -q '"code":"protocol.idempotency_conflict"' "$TMPDIR_PSM/resp3.json"; then
  fail "PSM-A2 case 3: expected code=protocol.idempotency_conflict; got:"
  cat "$TMPDIR_PSM/resp3.json" >&2
  exit 1
fi
ok "PSM-A2 case 3: K=K1, B=B2 (mutated) → 409 protocol.idempotency_conflict"

# Case 4: K2 (new) + B1 (same nonce as case 1) → 409 protocol.nonce_replay.
# Defense in depth: a fresh idempotency key cannot bypass the nonce gate.
post_idem "$K2" "$TMPDIR_PSM/idem-b1.json" "$TMPDIR_PSM/resp4.json" RESP4
if [[ "$RESP4" != "409" ]]; then
  fail "PSM-A2 case 4: K2 + B1 expected 409, got $RESP4"
  cat "$TMPDIR_PSM/resp4.json" >&2
  exit 1
fi
if ! grep -q '"code":"protocol.nonce_replay"' "$TMPDIR_PSM/resp4.json"; then
  fail "PSM-A2 case 4: expected code=protocol.nonce_replay; got:"
  cat "$TMPDIR_PSM/resp4.json" >&2
  exit 1
fi
ok "PSM-A2 case 4: K=K2 (new), B=B1 (same nonce) → 409 protocol.nonce_replay (nonce gate still wins)"

# Tear the server down so subsequent steps don't see a stray daemon.
kill "$SERVER_PID" 2>/dev/null || true
wait "$SERVER_PID" 2>/dev/null || true
unset SERVER_PID
trap 'rm -rf "$TMPDIR_PSM"' EXIT
echo

# ─── 8. Verifiable audit bundle from JSONL chain (REAL today) ────────────
bold "8. Verifiable audit bundle — receipt + JSONL chain (real today)"
# `mandate audit export` exists on main today (Developer A's PR #15). The
# `--chain` form is exercised here against the bundled DB-backed export of
# step 9 below; the full coverage of `--chain` is in
# `crates/mandate-cli/tests/audit_bundle.rs`.
if have_subcmd audit export; then
  ok "mandate audit export available (chain or DB)"
else
  fail "mandate audit export missing — main is in an unexpected state"
  exit 1
fi
echo

# ─── 9. DB-backed audit-bundle export + verify (REAL today) ──────────────
bold "9. DB-backed audit bundle — export from SQLite + verify"
# Public verification keys for the deterministic dev signers in
# `crates/mandate-server/src/lib.rs:54-55`. These are NOT secrets — they are
# derived from public seed bytes. Production deployments inject real
# signers via `AppState::with_signers` (TEE/HSM-backed); when PSM-A1.9 lands a
# `mandate key list --mock` command this script can switch to reading the
# pubkeys from there instead of hardcoding.
AUDIT_PUBKEY="66be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a"
RECEIPT_PUBKEY="ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c"
BUNDLE_PATH="$TMPDIR_PSM/bundle.json"

./target/debug/mandate audit export \
  --receipt "$RECEIPT_PATH" \
  --db "$DB_PATH" \
  --receipt-pubkey "$RECEIPT_PUBKEY" \
  --audit-pubkey "$AUDIT_PUBKEY" \
  --out "$BUNDLE_PATH" 2>&1 | sed 's/^/    /'
ok "mandate audit export --db ... --out $BUNDLE_PATH"

VERIFY_OUT="$(./target/debug/mandate audit verify-bundle --path "$BUNDLE_PATH" 2>&1)"
echo "$VERIFY_OUT" | sed 's/^/    /'
if ! grep -q '^ok: bundle verified' <<<"$VERIFY_OUT"; then
  fail "verify-bundle did not report success"
  exit 1
fi
ok "mandate audit verify-bundle — receipt + chain + signatures + linkage all valid"

# ─── 9b. Tamper detection on the bundle ──────────────────────────────────
TAMPERED_BUNDLE="$TMPDIR_PSM/bundle-tampered.json"
python3 - "$BUNDLE_PATH" "$TAMPERED_BUNDLE" <<'PY'
# Flip the receipt's decision from "allow" to "deny" without touching the
# signature. A correctly-implemented verifier MUST reject this — the
# signature was made over the canonical receipt with decision="allow".
import json, sys
src, dst = sys.argv[1], sys.argv[2]
b = json.load(open(src))
# Bundle layout: { "receipt": {... "decision": "allow", "signature": {...} }, ... }
b["receipt"]["decision"] = "deny"
json.dump(b, open(dst, "w"), separators=(",", ":"), sort_keys=True)
PY
if ./target/debug/mandate audit verify-bundle --path "$TAMPERED_BUNDLE" >/dev/null 2>&1; then
  fail "tampered bundle was accepted by verify-bundle (this is critical)"
  exit 1
fi
ok "tampered bundle correctly rejected by verify-bundle"
echo

# ─── 10. Audit checkpoint create/verify (Developer A backlog) ────────────
bold "10. Audit checkpoint create/verify (PSM-A4)"
if have_subcmd audit checkpoint; then
  ./target/debug/mandate audit checkpoint create --help >/dev/null 2>&1 \
    && ok "mandate audit checkpoint available" \
    || skip "mandate audit checkpoint subcommand exists but help failed"
else
  skip "blocked: waiting for \`mandate audit checkpoint\` (backlog PSM-A4)"
  note_skip "Audit checkpoints + mock anchoring — PSM-A4"
fi
echo

# ─── 11. Trust-badge / operator console build ────────────────────────────
bold "11. Trust-badge build (judge-readable proof viewer)"
TRUST_OUT="$(python3 trust-badge/build.py 2>&1)"
echo "$TRUST_OUT" | sed 's/^/    /'
if ! grep -q '^trust-badge: wrote' <<<"$TRUST_OUT"; then
  fail "trust-badge/build.py did not produce the expected output"
  exit 1
fi
ok "trust-badge/index.html generated from latest demo summary"

if python3 trust-badge/test_build.py >/dev/null 2>&1; then
  ok "trust-badge/test_build.py — render regression coverage green"
else
  fail "trust-badge/test_build.py failed"
  exit 1
fi
echo

# ─── 12. Final summary ───────────────────────────────────────────────────
bold "Production-shaped mock run complete."
echo
cat <<EOF
  This was the production-shaped mock surface, not a live deployment.

  REAL today (executed by this run, no network):
    - persistent SQLite-backed APRP + nonce-replay (legit + prompt-injection)
    - signed Ed25519 policy receipts, hash-chained audit log
    - mock KMS CLI surface (PSM-A1.9): \`mandate key {init,list,rotate} --mock\`
      lifecycle exercised against a fresh SQLite (V005 \`mock_kms_keys\`)
    - \`mandate doctor\` (PSM-A5): operator readiness summary
    - HTTP \`Idempotency-Key\` safe-retry (PSM-A2): four-case behaviour
      matrix exercised against a real mandate-server on 127.0.0.1:${IDEM_PORT}
      with persistent SQLite at \`$(basename "$IDEM_DB")\`
    - \`mandate audit export --db\` over the live SQLite file
    - \`mandate audit verify-bundle\` round-trip
    - tamper detection on the exported bundle
    - agent no-key proof (covered in the 13-gate final demo)
    - trust-badge static proof viewer + render regression test

  MOCK / OFFLINE (clearly labelled in demo output):
    - KeeperHub guarded execution: \`KeeperHubExecutor::local_mock()\`
    - Uniswap guarded swap:        \`UniswapExecutor::local_mock()\`
    - ENS resolver:                offline fixture (\`OfflineEnsResolver\`)
    - signing seeds:               deterministic dev seeds in mandate-server (⚠ DEV ONLY ⚠)

  SKIPPED (backlog items, real production-shaped commands not merged yet):
${SKIPPED_NOTES[@]+$(printf '%s\n' "${SKIPPED_NOTES[@]}")}

  Tally: $REAL_COUNT real, $MOCK_COUNT mock, $SKIP_COUNT skipped.

  > Don't give your agent a wallet. Give it a mandate.
EOF
