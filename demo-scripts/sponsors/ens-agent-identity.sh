#!/usr/bin/env bash
# Sponsor demo: ENS as agent identity for SBO3L.
#
# Default (offline, CI-deterministic): resolves `research-agent.team.eth` from
# the local ENS fixture, prints the `sbo3l:*` text records, and verifies that
# the published `sbo3l:policy_hash` matches the canonical hash of the active
# reference policy.
#
# `--live` mode: hits the real ENS Public Resolver via `LiveEnsResolver` (in
# `crates/sbo3l-identity/src/ens_live.rs`) using the operator-supplied
# `SBO3L_ENS_RPC_URL`. Reads the same five `sbo3l:*` records — but from a real
# ENS name on real Ethereum mainnet (default: `sbo3lagent.eth`, owned by the
# team and provisioned with all five records during the submission window).
# Operator override: `SBO3L_ENS_NAME=<name>`.
set -euo pipefail
cd "$(dirname "$0")/../.."

LIVE=0
if [[ "${1:-}" == "--live" ]]; then
  LIVE=1
fi

bold() { printf '\033[1m%s\033[0m\n' "$1"; }

if [[ "$LIVE" == "1" ]]; then
  if [[ -z "${SBO3L_ENS_RPC_URL:-}" ]]; then
    echo "ERROR: --live mode requires SBO3L_ENS_RPC_URL (mainnet JSON-RPC endpoint)" >&2
    echo "       Default offline mode runs without it; pass --live only if you have an RPC URL." >&2
    exit 2
  fi
  ENS_NAME="${SBO3L_ENS_NAME:-sbo3lagent.eth}"
  bold "ENS agent identity (LIVE) — $ENS_NAME via real ENS Public Resolver"
  echo "  Resolver:    LiveEnsResolver"
  echo "  RPC URL:     [redacted from log]"
  echo "  Name:        $ENS_NAME"
  echo
  cargo run --quiet -p sbo3l-identity --example ens_live_smoke
else
  cargo build --quiet --bin research-agent
  bold "ENS agent identity (mock fixture) — research-agent.team.eth"
  echo "  (default; pass --live with SBO3L_ENS_RPC_URL set to hit real mainnet ENS)"
  echo
  ./demo-agents/research-agent/run \
    --scenario legit-x402 \
    --ens-fixture demo-fixtures/ens-records.json \
    --ens-name research-agent.team.eth
fi
