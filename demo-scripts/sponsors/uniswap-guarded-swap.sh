#!/usr/bin/env bash
# Sponsor demo: Uniswap guarded swap for SBO3L.
#
# SBO3L is not a trading bot. The Uniswap adapter exists to prove that an
# agent which wants to trade through Uniswap can still be bounded by SBO3L's
# policy boundary. Two paths:
#
#   1. Allow path  — USDC -> ETH within token allowlist, max notional, max
#      slippage, freshness window and treasury recipient. SBO3L signs an
#      `allow` receipt; the Uniswap mock executor returns a `uni-<ULID>`
#      execution_ref.
#   2. Deny path   — USDC -> RUG, 1500 bps slippage, attacker recipient. The
#      swap-policy guard flags multiple violations AND SBO3L's policy
#      engine denies (`policy.deny_recipient_not_allowlisted`). The Uniswap
#      executor refuses to run on the denied receipt.
#
# B7: `UniswapExecutor::live()` now wires to the real Sepolia QuoterV2
# (0xEd1f6473345F45b75F8179591dd5bA1888cf2FB3). Default behaviour is mock
# for CI determinism. To exercise the live path locally:
#
#   export SBO3L_UNISWAP_RPC_URL='https://sepolia.example/...'
#   export SBO3L_UNISWAP_TOKEN_OUT='0x...'   # required Sepolia ERC20 address
#   ./demo-scripts/sponsors/uniswap-guarded-swap.sh --live
#
# The `--live` segment runs the `uniswap_live_smoke` example, which calls
# `quoteExactInputSingle` via `UniswapExecutor::live_from_env()` and prints
# the four QuoterV2 return values. CI never runs this segment (no RPC URL).
set -euo pipefail
cd "$(dirname "$0")/../.."

LIVE_MODE=0
for arg in "$@"; do
  case "$arg" in
    --live) LIVE_MODE=1 ;;
    *) echo "uniswap-guarded-swap.sh: unknown arg: $arg" >&2; exit 2 ;;
  esac
done

cargo build --quiet --bin research-agent

bold() { printf '\033[1m%s\033[0m\n' "$1"; }
bold "Uniswap guarded swap — allow path (USDC -> ETH within caps)"
echo
./demo-agents/research-agent/run \
  --uniswap-quote demo-fixtures/uniswap/quote-USDC-ETH.json \
  --swap-policy demo-fixtures/uniswap/swap-policy.json \
  --policy demo-fixtures/uniswap/sbo3l-policy.json \
  --execute-uniswap
echo
bold "Uniswap guarded swap — deny path (USDC -> RUG, attacker recipient)"
echo
./demo-agents/research-agent/run \
  --uniswap-quote demo-fixtures/uniswap/quote-USDC-RUG.json \
  --swap-policy demo-fixtures/uniswap/swap-policy.json \
  --policy demo-fixtures/uniswap/sbo3l-policy.json \
  --execute-uniswap

echo
bold "Uniswap guarded swap — Passport capsule with executor_evidence (P6.1)"
echo
# Build the CLI binary, emit a Passport capsule for the allow-path APRP
# via UniswapExecutor::local_mock(), print the executor_evidence block,
# and round-trip-verify the capsule. The block should be a 10-field
# UniswapQuoteEvidence object (mock-prefixed quote_id, USDC->ETH route,
# 50 bps slippage cap, treasury sentinel `0x111…111`).
#
# The capsule's `live_evidence` slot stays null in mock mode — the
# verifier's bidirectional invariant (mock ⇒ no live_evidence) is
# unchanged by P6.1. Sponsor business evidence lives in the new
# mode-agnostic `executor_evidence` slot.
cargo build --quiet --bin sbo3l
TMPDIR_PASSPORT="$(mktemp -d -t sbo3l-uniswap-passport-XXXXXX)"
trap 'rm -rf "$TMPDIR_PASSPORT"' EXIT

DB="$TMPDIR_PASSPORT/m.db"
CAPSULE="$TMPDIR_PASSPORT/uniswap-capsule.json"
ARTIFACTS=demo-scripts/artifacts
TRANSCRIPT="$ARTIFACTS/uniswap-evidence-offline-verify.txt"
mkdir -p "$ARTIFACTS"

./target/debug/sbo3l policy activate \
  test-corpus/policy/reference_low_risk.json \
  --db "$DB" >/dev/null

./target/debug/sbo3l passport run \
  test-corpus/aprp/golden_001_minimal.json \
  --db "$DB" \
  --agent research-agent.team.eth \
  --resolver offline-fixture \
  --ens-fixture demo-fixtures/ens-records.json \
  --executor uniswap \
  --mode mock \
  --out "$CAPSULE"

# --------------------------------------------------------------------------
# A4 — offline post-hoc verification framing.
#
# Tee the evidence + verify segments to a transcript file so the demo
# video gets a deterministic snapshot. Most of the byte-content is
# stable across runs (executor_evidence shape, schema id, decision,
# matched_rule); the few non-deterministic fields (quote_id ULID,
# quote_timestamp_unix, request_hash, audit chain hashes) are NOT
# asserted on in the regression test — the test pins SHAPE, not
# byte-equality.
# --------------------------------------------------------------------------
{
  echo "== executor_evidence — what an offline auditor reads =="
  echo "  capsule.execution.executor_evidence (P6.1 — Uniswap quote evidence):"
  jq '.execution.executor_evidence' "$CAPSULE"
  echo
  echo "== passport verify — schema + 8 cross-field invariants, NO network =="
  ./target/debug/sbo3l passport verify --path "$CAPSULE"
  echo
  echo "== Why this is the differentiator =="
  cat <<'EOF'
  Other Uniswap-track entries show the swap going out. SBO3L shows the
  swap going out PLUS a signed proof artefact a third party can verify
  WITHOUT contacting any RPC, any Uniswap subgraph, any agent backend,
  or KeeperHub:

    - the capsule carries the JCS-canonical request_hash + the active
      policy_hash + the Ed25519 receipt signature + the audit-chain
      position;
    - executor_evidence carries the actual quote shape that ran (10
      fields: quote_id, quote_source, input/output token, route,
      notional_in, slippage_cap_bps, quote_timestamp_unix,
      quote_freshness_seconds, recipient_address);
    - `sbo3l passport verify --path <capsule>` re-verifies the schema
      and every cross-field invariant offline by default
      (deny→no execution, live→evidence, request/policy hash
      internal-consistency); to additionally re-derive cryptography
      from the capsule alone, pass `--strict --policy <file>
      --receipt-pubkey <hex> --audit-bundle <file>`. Both modes work
      with the agent's published Ed25519 pubkey only — no daemon, no
      network, no RPC.

  Offline. Post-hoc. Single file. That's the SBO3L story.
EOF
} | tee "$TRANSCRIPT"
echo
echo "  transcript → $TRANSCRIPT (ignored by demo-scripts/artifacts/.gitignore)"

# --------------------------------------------------------------------------
# B7 — optional live segment. Skipped unless `--live` is passed AND the
# SBO3L_UNISWAP_RPC_URL env var is set. CI does not invoke either, so the
# block above remains the always-green default.
# --------------------------------------------------------------------------
if [ "$LIVE_MODE" -eq 1 ]; then
  echo
  bold "Uniswap guarded swap — LIVE Sepolia QuoterV2 (B7)"
  echo
  if [ -z "${SBO3L_UNISWAP_RPC_URL:-}" ]; then
    echo "  --live requested, but SBO3L_UNISWAP_RPC_URL is unset. Skipping."
    echo "  Set the env var (and SBO3L_UNISWAP_TOKEN_OUT) and rerun to exercise"
    echo "  the live path. The mock segments above continue to work without it."
  else
    echo "  RPC: $SBO3L_UNISWAP_RPC_URL"
    echo "  Quoter: 0xEd1f6473345F45b75F8179591dd5bA1888cf2FB3 (Sepolia QuoterV2)"
    echo
    cargo run --quiet -p sbo3l-execution --example uniswap_live_smoke
  fi
fi
