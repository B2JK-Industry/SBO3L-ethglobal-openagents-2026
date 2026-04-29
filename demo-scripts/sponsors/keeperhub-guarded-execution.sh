#!/usr/bin/env bash
# Sponsor demo: KeeperHub guarded execution for SBO3L.
#
# SBO3L decides, KeeperHub executes. Approved actions are routed to a
# KeeperHub local-mock executor; denied actions never reach the sponsor.
set -euo pipefail
cd "$(dirname "$0")/../.."

cargo build --quiet --bin research-agent

bold() { printf '\033[1m%s\033[0m\n' "$1"; }
bold "KeeperHub guarded execution — allow path (legit-x402)"
echo
./demo-agents/research-agent/run \
  --scenario legit-x402 \
  --execute-keeperhub
echo
bold "KeeperHub guarded execution — deny path (prompt-injection)"
echo
./demo-agents/research-agent/run \
  --scenario prompt-injection \
  --execute-keeperhub
