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
# The live executor (`UniswapExecutor::live()`) is intentionally stubbed in
# this hackathon build and would error with `BackendOffline`. There is no
# env-var feature flag — the demo always uses `local_mock()`. See FEEDBACK.md.
set -euo pipefail
cd "$(dirname "$0")/../.."

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

echo
echo "  capsule.execution.executor_evidence (P6.1 — Uniswap quote evidence):"
jq '.execution.executor_evidence' "$CAPSULE"

echo
./target/debug/sbo3l passport verify --path "$CAPSULE"
