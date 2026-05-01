#!/usr/bin/env bash
#
# T-3-3 demo — resolve every agent in a fleet manifest via a public
# RPC and assert each agent's `sbo3l:agent_id` text record matches the
# manifest. <5s end-to-end against PublicNode (no API key needed).
#
# Usage:
#   ./scripts/resolve-fleet.sh docs/proof/ens-fleet-2026-05-01.json
#
# Env (optional):
#   SBO3L_RESOLVE_RPC_URL  — JSON-RPC endpoint to resolve against.
#                            Defaults per the manifest's `network`:
#                            mainnet → ethereum-rpc.publicnode.com
#                            sepolia → ethereum-sepolia-rpc.publicnode.com
#
# Exit codes:
#   0  every agent resolved with the expected agent_id
#   1  IO / network error
#   2  manifest argument missing or malformed
#   3  one or more agents failed to resolve

set -euo pipefail

MANIFEST_PATH="${1:-}"
if [ -z "$MANIFEST_PATH" ] || [ ! -f "$MANIFEST_PATH" ]; then
    echo "ERROR: usage: $0 <manifest.json>" >&2
    exit 2
fi

NETWORK=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['network'])" "$MANIFEST_PATH")
RPC_URL="${SBO3L_RESOLVE_RPC_URL:-}"
if [ -z "$RPC_URL" ]; then
    case "$NETWORK" in
        mainnet) RPC_URL="https://ethereum-rpc.publicnode.com" ;;
        sepolia) RPC_URL="https://ethereum-sepolia-rpc.publicnode.com" ;;
        *)       echo "ERROR: unknown network in manifest: $NETWORK" >&2; exit 2 ;;
    esac
fi

if ! command -v cast >/dev/null 2>&1; then
    echo "ERROR: \`cast\` (Foundry) not on PATH." >&2
    exit 2
fi

START_TS=$(python3 -c "import time; print(time.time())")

AGENT_COUNT=$(python3 -c "import json,sys; print(len(json.load(open(sys.argv[1]))['agents']))" "$MANIFEST_PATH")
SUCCESS=0
FAIL=0

i=0
while [ "$i" -lt "$AGENT_COUNT" ]; do
    FQDN=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['agents'][int(sys.argv[2])]['fqdn'])" "$MANIFEST_PATH" "$i")
    EXPECTED_AGENT_ID=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['agents'][int(sys.argv[2])]['agent_id'])" "$MANIFEST_PATH" "$i")

    ACTUAL=$(cast text "$FQDN" sbo3l:agent_id --rpc-url "$RPC_URL" 2>/dev/null || echo "")

    if [ "$ACTUAL" = "$EXPECTED_AGENT_ID" ]; then
        echo "  OK   $FQDN → $ACTUAL"
        SUCCESS=$((SUCCESS+1))
    else
        echo "  FAIL $FQDN expected '$EXPECTED_AGENT_ID' got '$ACTUAL'"
        FAIL=$((FAIL+1))
    fi
    i=$((i+1))
done

END_TS=$(python3 -c "import time; print(time.time())")
ELAPSED=$(python3 -c "print(f'{float($END_TS) - float($START_TS):.2f}')")

echo
echo "==================================================================="
echo "  $AGENT_COUNT agents | $SUCCESS resolved | $FAIL failed | ${ELAPSED}s"
echo "==================================================================="

if [ "$FAIL" -gt 0 ]; then
    exit 3
fi
exit 0
