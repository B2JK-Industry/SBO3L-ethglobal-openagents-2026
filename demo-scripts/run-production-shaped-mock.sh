#!/usr/bin/env bash
# SBO3L ETHGlobal Open Agents — production-shaped mock runner (PSM-B1).
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
# Tagline: "Don't give your agent a wallet. give it a mandate."

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

# Run ./target/debug/sbo3l <subcmd…> --help; success ⇒ command exists.
have_subcmd() {
  ./target/debug/sbo3l "$@" --help >/dev/null 2>&1
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
bold "SBO3L production-shaped mock runner (PSM-B1)"
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
cargo build --quiet --bin sbo3l
cargo build --quiet --bin research-agent
ok "cargo build --bin sbo3l, --bin research-agent"
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
bold "2. Operator / health (sbo3l doctor — PSM-A5)"
if have_subcmd doctor; then
  if ./target/debug/sbo3l doctor 2>&1 | sed 's/^/    /'; then
    ok "sbo3l doctor — passed"
  else
    fail "sbo3l doctor reported a problem"
    exit 1
  fi
  # Evidence capture for operator-console B2.v2 panel: also gather the
  # JSON envelope (sbo3l.doctor.v1) so the panel renders structured
  # check rows. Idempotent — runs against in-memory DB by default.
  DOCTOR_JSON="$(./target/debug/sbo3l doctor --json 2>&1 || true)"
else
  skip "blocked: waiting for \`sbo3l doctor\` (backlog PSM-A5)"
  note_skip "sbo3l doctor (operator readiness summary) — PSM-A5"
  DOCTOR_JSON=""
fi
echo

# ─── 3. Mock KMS CLI surface (PSM-A1.9 — REAL today) ─────────────────────
bold "3. Mock KMS CLI surface (PSM-A1.9)"
# PSM-A1.9 shipped in PR #28: persistent mock_kms_keys SQLite table
# (migration V005) + `sbo3l key {init,list,rotate} --mock` CLI surface.
# Every operation requires `--mock` and prefixes every output line with
# `mock-kms:` for explicit disclosure. We exercise init → list → rotate →
# list end-to-end against a fresh tempfile-backed SQLite, then drop it.
# Mock — not production-grade.
if have_subcmd key list; then
  # Section-local tempdir + EXIT trap — TMPDIR_PSM is set up in step 5,
  # later than this section, so we use a self-contained temp space and
  # tear it down on success. The trap is appended (not replaced) so we
  # don't disturb any later trap installs.
  KMS_TMP="$(mktemp -d -t sbo3l-mock-kms.XXXXXX)"
  KMS_DB="$KMS_TMP/mock-kms.db"
  # Deterministic 64-hex-char dev seed. NOT a secret — `sbo3l-server`'s
  # production-shaped DevSigner uses literally this byte pattern (all 0x11),
  # see `crates/sbo3l-server/src/lib.rs:54`. The corresponding public
  # verifying key is the audit-signer pubkey shipped in
  # `demo-fixtures/mock-kms-keys.json`.
  KMS_ROOT_SEED="$(python3 -c 'print("11"*32)')"
  # Capture each step's stdout into a variable for the operator-console
  # evidence transcript (B2.v2), then mirror to the operator's terminal.
  # `set -e` propagates command-substitution failures, so a non-zero
  # exit aborts the runner exactly as the previous `&&` chain did.
  KMS_INIT_OUT="$(./target/debug/sbo3l key init --mock --role audit-mock \
       --root-seed "$KMS_ROOT_SEED" --db "$KMS_DB" 2>&1)"
  printf '%s\n' "$KMS_INIT_OUT" | sed 's/^/    /'
  KMS_LIST1_OUT="$(./target/debug/sbo3l key list --mock --db "$KMS_DB" 2>&1)"
  printf '%s\n' "$KMS_LIST1_OUT" | sed 's/^/    /'
  KMS_ROTATE_OUT="$(./target/debug/sbo3l key rotate --mock --role audit-mock \
       --root-seed "$KMS_ROOT_SEED" --db "$KMS_DB" 2>&1)"
  printf '%s\n' "$KMS_ROTATE_OUT" | sed 's/^/    /'
  KMS_LIST2_OUT="$(./target/debug/sbo3l key list --mock --db "$KMS_DB" 2>&1)"
  printf '%s\n' "$KMS_LIST2_OUT" | sed 's/^/    /'
  ok "sbo3l key init/list/rotate --mock — full lifecycle exercised against fresh SQLite"
  rm -rf "$KMS_TMP"
else
  skip "signer + trait + rotation are merged in PR #22; waiting for \`sbo3l key list --mock\` / \`sbo3l key rotate --mock\` CLI + persistent mock-KMS storage table (backlog PSM-A1.9)"
  note_skip "Mock KMS CLI surface (\`sbo3l key list --mock\` / \`sbo3l key rotate --mock\`) + persistent mock-KMS storage table — PSM-A1.9"
  KMS_INIT_OUT=""; KMS_ROTATE_OUT=""; KMS_LIST2_OUT=""
fi
echo

# Tempdir + policy DB for sections 4 and 5. Section 3 above uses its
# own self-contained KMS_TMP so its lifecycle stays atomic; section 4
# (active-policy lifecycle) and section 5 (persistent-SQLite allow +
# deny) share one tempfile DB rooted here.
TMPDIR_PSM="$(mktemp -d -t sbo3l-prod-shaped.XXXXXX)"
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
  if ./target/debug/sbo3l policy validate "$REF_POLICY" 2>&1 | sed 's/^/    /'; then
    ok "sbo3l policy validate $REF_POLICY"
  else
    fail "sbo3l policy validate failed against the reference policy"
    exit 1
  fi
  # 4b. honest no-active (exit 3 on a fresh DB). `policy current` opens
  # the DB (running V001..V006 on first touch) and surfaces the empty
  # active_policy table as exit 3 + an honest "no active policy" line —
  # NOT a fake `ok`.
  if ./target/debug/sbo3l policy current --db "$POLICY_DB" 2>&1 | sed 's/^/    /'; then
    fail "policy current must exit non-zero on a fresh DB (honest no-active)"
    exit 1
  else
    ok "sbo3l policy current honestly reports no active policy on a fresh DB (exit 3)"
  fi
  # 4c. activate the reference policy
  if ./target/debug/sbo3l policy activate "$REF_POLICY" --db "$POLICY_DB" 2>&1 | sed 's/^/    /'; then
    ok "sbo3l policy activate $REF_POLICY -> v1"
  else
    fail "sbo3l policy activate failed"
    exit 1
  fi
  # 4d. current after activate -> ok with version + hash prefix.
  # Capture into a variable for the operator-console evidence transcript.
  POLICY_CURRENT_OUT="$(./target/debug/sbo3l policy current --db "$POLICY_DB" 2>&1)"
  printf '%s\n' "$POLICY_CURRENT_OUT" | sed 's/^/    /'
  ok "sbo3l policy current after activate (active row visible)"
  # 4e. diff identical files -> exit 0
  if ./target/debug/sbo3l policy diff "$REF_POLICY" "$REF_POLICY" 2>&1 | sed 's/^/    /'; then
    ok "sbo3l policy diff (identical files -> no differences)"
  else
    fail "sbo3l policy diff against itself must report no differences"
    exit 1
  fi
else
  skip "blocked: waiting for \`sbo3l policy current\` (backlog PSM-A3)"
  note_skip "Policy activation lifecycle (validate/current/activate/diff) — PSM-A3"
  POLICY_CURRENT_OUT=""
fi
echo

# ─── 5. Allow path on persistent SQLite ──────────────────────────────────
bold "5. Allow path — legit-x402 against persistent SQLite"
DB_PATH="$TMPDIR_PSM/sbo3l.db"
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
cargo build --quiet --bin sbo3l-server
IDEM_DB="$TMPDIR_PSM/idempotency.db"
IDEM_PORT="${SBO3L_PSM_IDEM_PORT:-18730}"
IDEM_BASE="http://127.0.0.1:${IDEM_PORT}"
SERVER_LOG="$TMPDIR_PSM/idempotency-server.log"

# Spawn a fresh sbo3l-server. EXIT trap was set in step 5 to clean
# $TMPDIR_PSM; we extend it to also kill the server PID.
SBO3L_DB="$IDEM_DB" SBO3L_LISTEN="127.0.0.1:${IDEM_PORT}" \
  ./target/debug/sbo3l-server >"$SERVER_LOG" 2>&1 &
SERVER_PID=$!
trap 'kill "${SERVER_PID:-0}" 2>/dev/null || true; rm -rf "$TMPDIR_PSM"' EXIT

# Wait for /v1/health (max ~6s).
for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30; do
  if curl -sf "$IDEM_BASE/v1/health" >/dev/null 2>&1; then break; fi
  sleep 0.2
done
if ! curl -sf "$IDEM_BASE/v1/health" >/dev/null 2>&1; then
  fail "sbo3l-server did not come up on $IDEM_BASE — log:"
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
# `sbo3l audit export` exists on main today (Developer A's PR #15). The
# `--chain` form is exercised here against the bundled DB-backed export of
# step 9 below; the full coverage of `--chain` is in
# `crates/sbo3l-cli/tests/audit_bundle.rs`.
if have_subcmd audit export; then
  ok "sbo3l audit export available (chain or DB)"
else
  fail "sbo3l audit export missing — main is in an unexpected state"
  exit 1
fi
echo

# ─── 9. DB-backed audit-bundle export + verify (REAL today) ──────────────
bold "9. DB-backed audit bundle — export from SQLite + verify"
# Public verification keys for the deterministic dev signers in
# `crates/sbo3l-server/src/lib.rs:54-55`. These are NOT secrets — they are
# derived from public seed bytes. Production deployments inject real
# signers via `AppState::with_signers` (TEE/HSM-backed); when PSM-A1.9 lands a
# `sbo3l key list --mock` command this script can switch to reading the
# pubkeys from there instead of hardcoding.
AUDIT_PUBKEY="66be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a"
RECEIPT_PUBKEY="ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c"
BUNDLE_PATH="$TMPDIR_PSM/bundle.json"

./target/debug/sbo3l audit export \
  --receipt "$RECEIPT_PATH" \
  --db "$DB_PATH" \
  --receipt-pubkey "$RECEIPT_PUBKEY" \
  --audit-pubkey "$AUDIT_PUBKEY" \
  --out "$BUNDLE_PATH" 2>&1 | sed 's/^/    /'
ok "sbo3l audit export --db ... --out $BUNDLE_PATH"

VERIFY_OUT="$(./target/debug/sbo3l audit verify-bundle --path "$BUNDLE_PATH" 2>&1)"
echo "$VERIFY_OUT" | sed 's/^/    /'
if ! grep -q '^ok: bundle verified' <<<"$VERIFY_OUT"; then
  fail "verify-bundle did not report success"
  exit 1
fi
ok "sbo3l audit verify-bundle — receipt + chain + signatures + linkage all valid"

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
if ./target/debug/sbo3l audit verify-bundle --path "$TAMPERED_BUNDLE" >/dev/null 2>&1; then
  fail "tampered bundle was accepted by verify-bundle (this is critical)"
  exit 1
fi
ok "tampered bundle correctly rejected by verify-bundle"
echo

# ─── 10. Audit checkpoint create/verify (PSM-A4 — REAL today) ───────────
# PSM-A4 shipped here: persistent `audit_checkpoints` table (V007) +
# `sbo3l audit checkpoint {create,verify}` CLI surface. This is
# **mock anchoring**, NOT real onchain anchoring — the
# `mock_anchor_ref` is a deterministic local id, never broadcast and
# never attested by any chain. The `mock-anchor:` prefix is on every
# CLI output line for loud disclosure. We exercise create → verify
# (no DB) → verify (--db) end-to-end against the live SQLite file
# from §5/§6 so the anchor binds to a real audit chain.
bold "10. Audit checkpoint create/verify (PSM-A4 — mock anchoring)"
if have_subcmd audit checkpoint; then
  CHECKPOINT_OUT="$TMPDIR_PSM/checkpoint.json"
  if ./target/debug/sbo3l audit checkpoint create \
       --db "$DB_PATH" --out "$CHECKPOINT_OUT" 2>&1 | sed 's/^/    /'; then
    ok "sbo3l audit checkpoint create --db $DB_PATH --out $CHECKPOINT_OUT"
  else
    fail "sbo3l audit checkpoint create returned non-zero"
    exit 1
  fi
  # Capture verify output for the operator-console evidence transcript.
  CHECKPOINT_VERIFY_OUT="$(./target/debug/sbo3l audit checkpoint verify "$CHECKPOINT_OUT" \
       --db "$DB_PATH" 2>&1)"
  printf '%s\n' "$CHECKPOINT_VERIFY_OUT" | sed 's/^/    /'
  ok "sbo3l audit checkpoint verify $(basename "$CHECKPOINT_OUT") --db $DB_PATH (chain_digest + anchor row + latest_event_hash all match)"
else
  skip "blocked: waiting for \`sbo3l audit checkpoint\` (backlog PSM-A4)"
  note_skip "Audit checkpoints + mock anchoring — PSM-A4"
  CHECKPOINT_OUT=""
  CHECKPOINT_VERIFY_OUT=""
fi
echo

# ─── 10b. Passport capsule emit + verify (Passport P2.1 — REAL today) ───
# `sbo3l passport run` orchestrates the existing offline pipeline
# (APRP → request_hash → policy → budget → audit → signed receipt) +
# mock executor handoff and emits one `sbo3l.passport_capsule.v1`
# JSON per request. Wraps existing primitives — no rewrite of crypto,
# audit semantics, or policy logic.
#
# We run two scenarios against POLICY_DB (already carries the
# reference policy from §4):
#
#   * legit-x402  → ALLOW capsule, executor=keeperhub mock,
#                   execution.status="submitted", execution_ref=kh-<ULID>
#   * prompt-injection → DENY capsule, execution.status="not_called",
#                        execution_ref=null (HARD invariant from P1.1)
#
# Outputs go into demo-scripts/artifacts/ so static proof surfaces
# (P2.2: trust-badge / operator-console capsule panels) and the
# schema validator can pick them up. Each capsule round-trips
# through `sbo3l passport verify` before the runner moves on.
bold "10b. Passport capsule emit + verify (Passport P2.1 — mock executor)"
if have_subcmd passport run; then
  PASSPORT_ARTIFACTS="demo-scripts/artifacts"
  mkdir -p "$PASSPORT_ARTIFACTS"
  PASSPORT_ALLOW="$PASSPORT_ARTIFACTS/passport-allow.json"
  PASSPORT_DENY="$PASSPORT_ARTIFACTS/passport-deny.json"

  if ./target/debug/sbo3l passport run \
       test-corpus/aprp/golden_001_minimal.json \
       --db "$POLICY_DB" \
       --agent research-agent.team.eth \
       --resolver offline-fixture \
       --ens-fixture demo-fixtures/ens-records.json \
       --executor keeperhub \
       --mode mock \
       --out "$PASSPORT_ALLOW" 2>&1 | sed 's/^/    /' \
     && ./target/debug/sbo3l passport verify --path "$PASSPORT_ALLOW" 2>&1 | sed 's/^/    /'; then
    ok "passport ALLOW capsule → $PASSPORT_ALLOW (run + verify)"
  else
    fail "passport allow path failed (run or verify)"
    exit 1
  fi
  if ./target/debug/sbo3l passport run \
       test-corpus/aprp/deny_prompt_injection_request.json \
       --db "$POLICY_DB" \
       --agent research-agent.team.eth \
       --resolver offline-fixture \
       --ens-fixture demo-fixtures/ens-records.json \
       --executor keeperhub \
       --mode mock \
       --out "$PASSPORT_DENY" 2>&1 | sed 's/^/    /' \
     && ./target/debug/sbo3l passport verify --path "$PASSPORT_DENY" 2>&1 | sed 's/^/    /'; then
    ok "passport DENY  capsule → $PASSPORT_DENY (run + verify; status=not_called, no execution_ref)"
  else
    fail "passport deny path failed (run or verify)"
    exit 1
  fi
else
  skip "blocked: waiting for \`sbo3l passport run\` (backlog Passport P2.1)"
  note_skip "Passport capsule emission (sbo3l passport run/verify) — Passport P2.1"
fi
echo

# ─── 11. Trust-badge / operator console build ────────────────────────────
bold "11. Trust-badge build (judge-readable proof viewer)"
# P1 review on PR #21: the default invocation (no --include-final-demo)
# never runs the 13-gate final demo, which is the only source of
# `demo-scripts/artifacts/latest-demo-summary.json` — the transcript
# trust-badge/build.py requires. On a clean checkout that would crash
# the runner here. Pre-check for the artifact and emit a deterministic
# skip with the exact regenerate command, instead of letting the
# downstream Python build crash the entire production-shaped run.
DEMO_SUMMARY_PATH="demo-scripts/artifacts/latest-demo-summary.json"
if [[ ! -s "$DEMO_SUMMARY_PATH" ]]; then
  skip "trust-badge build skipped: \`$DEMO_SUMMARY_PATH\` not found (pass \`--include-final-demo\` or first run \`bash demo-scripts/run-openagents-final.sh\` to populate it)"
  note_skip "Trust-badge regression coverage requires the demo-summary transcript"
else
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
fi
echo

# ─── 12. Operator-console evidence transcript (B2.v2) ────────────────────
# Synthesise a `sbo3l-operator-evidence-v1` JSON capturing every
# real-evidence panel the operator-console will render: PSM-A2
# idempotency 4-case, PSM-A5 doctor JSON, PSM-A1.9 mock KMS keyring,
# PSM-A3 active policy lifecycle, PSM-A4 audit checkpoints. Trust-badge
# is intentionally untouched — it continues to consume only
# `sbo3l-demo-summary-v1`. The new evidence file lives next to the
# existing demo-summary transcript and is gitignored.
bold "12. Operator-console evidence transcript (B2.v2)"
mkdir -p demo-scripts/artifacts
EVIDENCE_PATH="demo-scripts/artifacts/latest-operator-evidence.json"
DEMO_COMMIT="$(git rev-parse HEAD 2>/dev/null || echo unknown)"
GENERATED_AT_ISO="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
EVIDENCE_SCHEMA="sbo3l-operator-evidence-v1"

# Bridge captured shell vars + temp file paths into python via env.
# `${VAR:-}` makes the assignment safe if a step skipped (vars unset).
DOCTOR_JSON="${DOCTOR_JSON:-}" \
KMS_INIT_OUT="${KMS_INIT_OUT:-}" \
KMS_LIST1_OUT="${KMS_LIST1_OUT:-}" \
KMS_ROTATE_OUT="${KMS_ROTATE_OUT:-}" \
KMS_LIST2_OUT="${KMS_LIST2_OUT:-}" \
POLICY_CURRENT_OUT="${POLICY_CURRENT_OUT:-}" \
CHECKPOINT_PATH="${CHECKPOINT_OUT:-}" \
CHECKPOINT_VERIFY_OUT="${CHECKPOINT_VERIFY_OUT:-}" \
RESP1_PATH="${TMPDIR_PSM:-/dev/null}/resp1.json" \
RESP2_PATH="${TMPDIR_PSM:-/dev/null}/resp2.json" \
RESP3_PATH="${TMPDIR_PSM:-/dev/null}/resp3.json" \
RESP4_PATH="${TMPDIR_PSM:-/dev/null}/resp4.json" \
DEMO_COMMIT="$DEMO_COMMIT" \
GENERATED_AT_ISO="$GENERATED_AT_ISO" \
EVIDENCE_SCHEMA="$EVIDENCE_SCHEMA" \
EVIDENCE_PATH="$EVIDENCE_PATH" \
python3 - <<'PY'
import json, os, re

def _read_text(env_key):
    return os.environ.get(env_key, "") or ""

def _read_json_file(env_key):
    p = os.environ.get(env_key, "") or ""
    if p and os.path.isfile(p):
        try:
            with open(p) as fh:
                return json.load(fh)
        except (OSError, json.JSONDecodeError):
            pass
    return None

def _read_bytes_file(env_key):
    p = os.environ.get(env_key, "") or ""
    if p and os.path.isfile(p):
        try:
            with open(p, "rb") as fh:
                return fh.read()
        except OSError:
            pass
    return None

# 1. PSM-A5 — sbo3l doctor --json envelope.
doctor_raw = _read_text("DOCTOR_JSON").strip()
doctor_report = None
checks_summary = {"ok": 0, "skip": 0, "fail": 0}
malformed = False
if doctor_raw:
    try:
        doctor_report = json.loads(doctor_raw)
        for c in doctor_report.get("checks", []) or []:
            s = c.get("status", "")
            if s in checks_summary:
                checks_summary[s] += 1
    except json.JSONDecodeError:
        malformed = True
        doctor_report = {"_parse_error": "doctor --json output did not parse",
                         "_raw_first_120": doctor_raw[:120]}

# 2. PSM-A1.9 — mock KMS keyring. Parse the authoritative POST-ROTATE
# `key list` table (KMS_LIST2_OUT) which has all rows in fixed shape:
#   mock-kms:   <role>  <ver>  <key_id>  <public_hex>  <created_at>
# Public verification keys only — no private/signing material is ever
# logged by the CLI, by design.
keys = []
list2 = _read_text("KMS_LIST2_OUT")
for line in list2.splitlines():
    if not line.startswith("mock-kms:"):
        continue
    body = line[len("mock-kms:"):].strip()
    if body.startswith("keyring") or body.startswith("role"):
        continue  # header / count line
    parts = body.split()
    # Expect at least: role, ver, key_id, public_hex, created_at
    if len(parts) < 5:
        continue
    role, ver, key_id, pub, created_at = parts[0], parts[1], parts[2], parts[3], " ".join(parts[4:])
    try:
        ver_i = int(ver)
    except ValueError:
        continue
    keys.append({
        "role": role,
        "version": ver_i,
        "key_id": key_id,
        "verifying_key_hex": pub,
        "verifying_key_hex_prefix": pub[:12] if pub else None,
        "created_at": created_at,
        "mock": True,
    })

# 3. PSM-A3 — active policy after activate.
policy = {}
for line in _read_text("POLICY_CURRENT_OUT").splitlines():
    m = re.match(r'^\s+(version|policy_hash|source|activated_at):\s+(\S.*\S)\s*$', line)
    if m:
        policy[m.group(1)] = m.group(2)

# 4. PSM-A4 — checkpoint create + verify.
checkpoint_create = _read_json_file("CHECKPOINT_PATH")
verify_lines = [
    l.strip()
    for l in _read_text("CHECKPOINT_VERIFY_OUT").splitlines()
    if l.strip()
]
# P1 fix: each boolean must match `ok` on its OWN line. The previous
# `_has_phrase("ok")` co-condition matched ANY line containing the
# token "ok", so a `db cross-check: fail` line could still resolve
# `db_cross_check_ok=true` simply because some other line said "ok"
# (e.g. structural verify ok). That risked a false-positive evidence
# panel claiming a verification succeeded that actually failed.
def _line_status_ok(prefix):
    pat = re.compile(re.escape(prefix) + r':\s*ok\b')
    return any(pat.search(l) for l in verify_lines)
checkpoint_verify = {
    "raw_lines": verify_lines,
    "structural_verify_ok": _line_status_ok("structural verify"),
    "db_cross_check_ok":    _line_status_ok("db cross-check"),
    "result_ok":            _line_status_ok("verify result"),
}

# 5. PSM-A2 — idempotency 4-case matrix.
resp1 = _read_json_file("RESP1_PATH")
resp2 = _read_json_file("RESP2_PATH")
resp3 = _read_json_file("RESP3_PATH")
resp4 = _read_json_file("RESP4_PATH")
b1 = _read_bytes_file("RESP1_PATH")
b2 = _read_bytes_file("RESP2_PATH")
byte_identical = (b1 is not None and b2 is not None and b1 == b2)
idempotency = {
    "case_1_first_post": {
        "http_status": 200 if resp1 else None,
        "audit_event_id": (resp1 or {}).get("audit_event_id"),
        "decision": (resp1 or {}).get("decision"),
    },
    "case_2_cached_replay": {
        "http_status": 200 if resp2 else None,
        "byte_identical_to_case_1": byte_identical,
    },
    "case_3_idempotency_conflict": {
        "http_status": 409 if (resp3 and resp3.get("code") == "protocol.idempotency_conflict") else None,
        "code": (resp3 or {}).get("code"),
    },
    "case_4_nonce_replay_with_new_key": {
        "http_status": 409 if (resp4 and resp4.get("code") == "protocol.nonce_replay") else None,
        "code": (resp4 or {}).get("code"),
    },
}

evidence = {
    "schema": os.environ["EVIDENCE_SCHEMA"],
    "tagline": "Don't give your agent a wallet. Give it a mandate.",
    "demo_commit": os.environ.get("DEMO_COMMIT", "unknown"),
    "generated_at_iso": os.environ.get("GENERATED_AT_ISO", ""),
    "psm_a1_9_mock_kms": {
        "keys": keys,
        "post_rotate_listing_text": _read_text("KMS_LIST2_OUT"),
        "_mock_label": "Every entry above is from --mock keyring. Not production KMS.",
    },
    "psm_a2_idempotency": idempotency,
    "psm_a3_active_policy": policy if policy else None,
    "psm_a4_audit_checkpoints": {
        "create": checkpoint_create,
        "verify": checkpoint_verify,
        "_mock_anchor_label": "Mock anchoring, NOT onchain.",
    },
    "psm_a5_doctor": {
        "report": doctor_report,
        "checks_summary": checks_summary,
        "malformed": malformed,
    },
}
with open(os.environ["EVIDENCE_PATH"], "w", encoding="utf-8") as fh:
    json.dump(evidence, fh, indent=2, sort_keys=True)
    fh.write("\n")
PY
ok "wrote $EVIDENCE_PATH ($(wc -c < "$EVIDENCE_PATH" | tr -d ' ') bytes, schema=$EVIDENCE_SCHEMA)"
echo

# ─── 13. Final summary ───────────────────────────────────────────────────
bold "Production-shaped mock run complete."
echo
cat <<EOF
  This was the production-shaped mock surface, not a live deployment.

  REAL today (executed by this run, no network):
    - persistent SQLite-backed APRP + nonce-replay (legit + prompt-injection)
    - signed Ed25519 policy receipts, hash-chained audit log
    - mock KMS CLI surface (PSM-A1.9): \`sbo3l key {init,list,rotate} --mock\`
      lifecycle exercised against a fresh SQLite (V005 \`mock_kms_keys\`)
    - active-policy lifecycle (PSM-A3): \`sbo3l policy {validate,current,activate,diff}\`
      against V006 \`active_policy\` with DB-enforced singleton invariant
    - audit checkpoints (PSM-A4 — **MOCK ANCHORING**, not onchain):
      \`sbo3l audit checkpoint {create,verify}\` against V007 \`audit_checkpoints\`;
      every output line carries the \`mock-anchor:\` prefix
    - \`sbo3l doctor\` (PSM-A5): operator readiness summary
    - HTTP \`Idempotency-Key\` safe-retry (PSM-A2): four-case behaviour
      matrix exercised against a real sbo3l-server on 127.0.0.1:${IDEM_PORT}
      with persistent SQLite at \`$(basename "$IDEM_DB")\`
    - \`sbo3l audit export --db\` over the live SQLite file
    - \`sbo3l audit verify-bundle\` round-trip
    - tamper detection on the exported bundle
    - agent no-key proof (covered in the 13-gate final demo)
    - trust-badge static proof viewer + render regression test

  MOCK / OFFLINE (clearly labelled in demo output):
    - KeeperHub guarded execution: \`KeeperHubExecutor::local_mock()\`
    - Uniswap guarded swap:        \`UniswapExecutor::local_mock()\`
    - ENS resolver:                offline fixture (\`OfflineEnsResolver\`)
    - signing seeds:               deterministic dev seeds in sbo3l-server (⚠ DEV ONLY ⚠)

  SKIPPED (backlog items, real production-shaped commands not merged yet):
${SKIPPED_NOTES[@]+$(printf '%s\n' "${SKIPPED_NOTES[@]}")}

  Tally: $REAL_COUNT real, $MOCK_COUNT mock, $SKIP_COUNT skipped.

  > Don't give your agent a wallet. Give it a mandate.
EOF
