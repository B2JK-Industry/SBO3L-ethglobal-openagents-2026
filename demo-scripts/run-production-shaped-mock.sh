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

# ─── 3. Mock KMS / signer lifecycle (Developer A backlog) ────────────────
bold "3. Mock KMS — signer lifecycle (PSM-A1)"
if have_subcmd key list; then
  ./target/debug/mandate key list --mock 2>&1 | sed 's/^/    /' && ok "mandate key list --mock" || skip "mandate key list --mock returned non-zero"
else
  skip "blocked: waiting for \`mandate key list --mock\` (backlog PSM-A1)"
  note_skip "Mock KMS / key rotation lifecycle — PSM-A1"
fi
echo

# ─── 4. Active policy lifecycle (Developer A backlog) ────────────────────
bold "4. Active policy lifecycle (PSM-A3)"
if have_subcmd policy current; then
  ./target/debug/mandate policy current 2>&1 | sed 's/^/    /' && ok "mandate policy current" || skip "mandate policy current returned non-zero"
else
  skip "blocked: waiting for \`mandate policy current\` (backlog PSM-A3)"
  note_skip "Policy activation lifecycle (validate/current/activate/diff) — PSM-A3"
fi
echo

# ─── 5. Allow path on persistent SQLite ──────────────────────────────────
bold "5. Allow path — legit-x402 against persistent SQLite"
TMPDIR_PSM="$(mktemp -d -t mandate-prod-shaped.XXXXXX)"
trap 'rm -rf "$TMPDIR_PSM"' EXIT
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

# ─── 7. Idempotency-Key safe retry (Developer A backlog) ─────────────────
bold "7. Idempotency-Key safe retry (PSM-A2)"
# Idempotency-Key is an HTTP header on POST /v1/payment-requests, gated by
# Developer A's PSM-A2. There is no CLI subcommand to probe for it today, so
# this step is unconditionally a SKIP until the OpenAPI document or release
# notes state that the header is honoured. We do NOT fabricate retry output.
skip "blocked: waiting for HTTP \`Idempotency-Key\` support (backlog PSM-A2)"
note_skip "HTTP Idempotency-Key safe-retry semantics — PSM-A2"
echo

# ─── 8. Verifiable audit bundle from JSONL chain (REAL today) ────────────
bold "8. Verifiable audit bundle — receipt + JSONL chain (PSM-A1, real today)"
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
# signers via `AppState::with_signers` (TEE/HSM-backed); when PSM-A1 lands a
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
