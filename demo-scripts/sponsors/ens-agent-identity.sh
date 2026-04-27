#!/usr/bin/env bash
# Sponsor demo: ENS as agent identity for Mandate.
#
# Resolves `research-agent.team.eth` from the local ENS fixture, prints the
# `mandate:*` text records, and verifies that the published `mandate:policy_hash`
# matches the canonical hash of the active reference policy.
set -euo pipefail
cd "$(dirname "$0")/../.."

cargo build --quiet --bin research-agent

bold() { printf '\033[1m%s\033[0m\n' "$1"; }
bold "ENS agent identity — research-agent.team.eth"
echo
./demo-agents/research-agent/run \
  --scenario legit-x402 \
  --ens-fixture demo-fixtures/ens-records.json \
  --ens-name research-agent.team.eth
