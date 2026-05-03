#!/usr/bin/env bash
#
# simulate-mainnet-deploy.sh — anvil-forked dry run of the full
# mainnet OR + 60-subname fleet deploy. Spends zero real ETH;
# reports per-step gas estimate, total cost, and final on-chain
# state Daniel would see post-broadcast.
#
# Companion to docs/dev4/mainnet-deploy-runbook.md (R20 Task A).
# Daniel reviews the simulator output BEFORE deciding whether to
# broadcast the real version.
#
# What it simulates (R21 Task C, 2026-05-03):
#
#   STEP 1  Deploy OffchainResolver on a mainnet fork
#   STEP 2  setResolver(sbo3lagent.eth, <new OR>) on the fork
#   STEP 3  60x setSubnodeRecord (50 numbered + 10 specialist) via
#           the existing forge script script/RegisterMainnetFleet.s.sol
#   Verify  read final state to confirm subnames exist + resolve
#           to the new OR
#
# Output (stdout):
#   Per-step status + gas estimate + cumulative cost
#   Final cost summary (steps 1+2+3 sum, +20% gas headroom)
#   Final state read-back (5 spot-checked subname namehashes)
#
# Usage:
#   ./scripts/simulate-mainnet-deploy.sh
#
# Required:
#   - anvil (foundry)
#   - cast (foundry)
#   - forge (foundry)
#   - MAINNET_RPC_URL env var pointing at an upstream mainnet RPC
#     to fork from. Optional override via $1 positional arg.
#
# What this DOES NOT do:
#   - Send any real mainnet tx (anvil fork is local + ephemeral)
#   - Test the gateway records.json side (that's seed-fleet-records.mjs;
#     simulator focuses on on-chain side)
#   - Check Daniel's actual mainnet wallet balance (the simulator
#     uses anvil's pre-funded test accounts; Daniel's wallet
#     headroom check lives in docs/dev4/mainnet-deploy-runbook.md
#     STEP 0)

set -euo pipefail

MAINNET_RPC_URL="${1:-${MAINNET_RPC_URL:-}}"
if [[ -z "$MAINNET_RPC_URL" ]]; then
    echo "ERROR: MAINNET_RPC_URL not set + no positional arg." >&2
    echo "       Pass an Alchemy/Infura/PublicNode mainnet RPC URL." >&2
    exit 2
fi

ANVIL_PORT=8545
ANVIL_PID=
ANVIL_LOG=/tmp/sbo3l-simulator-anvil.log
SIM_OUTPUT_DIR=/tmp/sbo3l-simulator-output
mkdir -p "$SIM_OUTPUT_DIR"

# Robust RPC redaction. Previous version assumed Alchemy's `/v2/`
# segment and would leak Infura `/v3/<key>`, generic
# `?apikey=<key>`, or QuickNode `/<token>/` cleartext (Codex P2 on
# PR #486 caught this). Strip everything past the host, replace
# with `/<redacted>`. Pure shell param expansion — no sed (BSD vs
# GNU regex inconsistencies).
redact_rpc() {
    local url="$1"
    local scheme="${url%%://*}"
    local rest="${url#*://}"
    local host_port="${rest%%/*}"
    host_port="${host_port%%\?*}"
    host_port="${host_port%%\#*}"
    printf '%s://%s/<redacted>' "$scheme" "$host_port"
}

cleanup() {
    if [[ -n "$ANVIL_PID" ]]; then
        kill "$ANVIL_PID" 2>/dev/null || true
        wait "$ANVIL_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT INT TERM

echo "==================================================================="
echo "  SBO3L mainnet deploy SIMULATOR — $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "  upstream RPC:  $(redact_rpc "$MAINNET_RPC_URL")"
echo "  fork port:     $ANVIL_PORT"
echo "  output dir:    $SIM_OUTPUT_DIR"
echo "==================================================================="
echo

echo "[1/4] Booting anvil mainnet fork..."
anvil \
    --fork-url "$MAINNET_RPC_URL" \
    --port "$ANVIL_PORT" \
    --silent > "$ANVIL_LOG" 2>&1 &
ANVIL_PID=$!
sleep 3

# Wait for anvil readiness
LOCAL_RPC="http://127.0.0.1:$ANVIL_PORT"
for i in 1 2 3 4 5; do
    if cast chain-id --rpc-url "$LOCAL_RPC" >/dev/null 2>&1; then
        break
    fi
    sleep 1
    if [[ "$i" == "5" ]]; then
        echo "ERROR: anvil never became ready. See $ANVIL_LOG" >&2
        exit 1
    fi
done

# Anvil pre-funded test account #0 (10000 ETH each) — used as the
# simulated deployer. Note this is NOT Daniel's real wallet; the
# simulator confirms the *flow* works, not that Daniel's specific
# wallet has perms. (Daniel's wallet IS the parent owner on mainnet
# real-state — the impersonation below handles that.)
SIM_DEPLOYER=0xf39Fd6e51aad88F6F4ce6aB8827279cfFFb92266
SIM_DEPLOYER_PK=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80

echo "[2/4] Pre-flight: confirm sbo3lagent.eth is owned + reachable"
NODE=$(cast namehash sbo3lagent.eth)
ENS_REGISTRY=0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e
PARENT_OWNER=$(cast call "$ENS_REGISTRY" "owner(bytes32)(address)" "$NODE" --rpc-url "$LOCAL_RPC")
PRIOR_RESOLVER=$(cast call "$ENS_REGISTRY" "resolver(bytes32)(address)" "$NODE" --rpc-url "$LOCAL_RPC")
echo "  parent node:         $NODE"
echo "  parent owner:        $PARENT_OWNER"
echo "  prior resolver:      $PRIOR_RESOLVER"

# Use anvil_impersonateAccount to broadcast as Daniel's actual
# mainnet wallet. This is an anvil-only superpower — on real
# mainnet only Daniel's PK can sign for that address.
echo
echo "  impersonating $PARENT_OWNER on the anvil fork..."
cast rpc anvil_impersonateAccount "$PARENT_OWNER" --rpc-url "$LOCAL_RPC" >/dev/null
cast rpc anvil_setBalance "$PARENT_OWNER" 0x21e19e0c9bab2400000 --rpc-url "$LOCAL_RPC" >/dev/null  # 10000 ETH

echo
echo "[3/4] Simulating deploy steps"
echo

# ============================================================
# STEP 1 — Deploy mainnet OffchainResolver
# ============================================================
echo "  STEP 1: forge create OffchainResolver"
GATEWAY_SIGNER=0x595099B4e8D642616e298235Dd1248f8008BCe65
DEPLOY_OUT=$(cd crates/sbo3l-identity/contracts && \
    PRIVATE_KEY=$SIM_DEPLOYER_PK \
    GATEWAY_SIGNER_ADDRESS=$GATEWAY_SIGNER \
    forge script script/DeployOffchainResolver.s.sol \
        --rpc-url "$LOCAL_RPC" \
        --broadcast \
        --silent \
        2>&1 || true)
NEW_OR=$(echo "$DEPLOY_OUT" | grep -oE "0x[0-9a-fA-F]{40}" | head -1)
if [[ -z "$NEW_OR" ]]; then
    echo "  ❌ failed to deploy OR. anvil log: $ANVIL_LOG" >&2
    echo "$DEPLOY_OUT" > "$SIM_OUTPUT_DIR/step1-deploy-or.log"
    exit 1
fi
STEP1_GAS=$(cast estimate --rpc-url "$LOCAL_RPC" --create "$(cast code "$NEW_OR" --rpc-url "$LOCAL_RPC")" 2>/dev/null || echo "1500000")
echo "  ✅ deployed OR at $NEW_OR"
echo "  STEP 1 gas estimate: ${STEP1_GAS} (~ \$$(awk "BEGIN { printf \"%.2f\", $STEP1_GAS * 50 / 1e9 * 4000 }"))"
echo

# ============================================================
# STEP 2 — setResolver on sbo3lagent.eth (impersonated)
# ============================================================
echo "  STEP 2: cast send setResolver(sbo3lagent.eth, $NEW_OR)"
STEP2_TX=$(cast send "$ENS_REGISTRY" \
    "setResolver(bytes32,address)" \
    "$NODE" "$NEW_OR" \
    --from "$PARENT_OWNER" \
    --unlocked \
    --rpc-url "$LOCAL_RPC" \
    --json 2>&1 || echo "{}")
STEP2_GAS=$(echo "$STEP2_TX" | python3 -c "import sys,json; d=json.loads(sys.stdin.read() or '{}'); print(int(d.get('gasUsed','0x14000'),16))" 2>/dev/null || echo "80000")
NEW_RESOLVER=$(cast call "$ENS_REGISTRY" "resolver(bytes32)(address)" "$NODE" --rpc-url "$LOCAL_RPC")
if [[ "${NEW_RESOLVER,,}" != "${NEW_OR,,}" ]]; then
    echo "  ❌ setResolver failed: resolver still $NEW_RESOLVER, expected $NEW_OR" >&2
    exit 1
fi
echo "  ✅ resolver flipped to $NEW_OR"
echo "  STEP 2 gas estimate: ${STEP2_GAS} (~ \$$(awk "BEGIN { printf \"%.2f\", $STEP2_GAS * 50 / 1e9 * 4000 }"))"
echo

# ============================================================
# STEP 3 — 60x setSubnodeRecord via cast send loop
# ============================================================
#
# WHY a cast loop instead of `forge script RegisterMainnetFleet`:
# the forge script derives its broadcaster from
# `vm.envUint("PRIVATE_KEY")` and asserts `parentOwner == deployer`
# inside `setUp()`. With `--unlocked --sender PARENT_OWNER` the
# foundry --sender flag does NOT bypass that assertion (the script
# still derives the address from PRIVATE_KEY), so the script
# reverts. cast send with `--unlocked --from PARENT_OWNER` against
# the impersonated address bypasses the foundry script layer
# entirely. (Codex P1 on PR #486 caught this.)
echo "  STEP 3: cast send setSubnodeRecord loop (60 subnames)"

# Build the same 60-label list RegisterMainnetFleet.s.sol uses
# (50 numbered + 10 specialist). Keep this list in lock-step with
# the .s.sol file when either changes.
ALL_LABELS=()
for i in $(seq 1 50); do
    ALL_LABELS+=("agent-$(printf '%03d' "$i")")
done
ALL_LABELS+=(research trader auditor compliance treasury analytics reputation oracle messenger executor)

FLEET_FAILS=0
STEP3_GAS_TOTAL=0
for label in "${ALL_LABELS[@]}"; do
    LABEL_HASH=$(cast keccak "$label")
    TX_OUT=$(cast send "$ENS_REGISTRY" \
        "setSubnodeRecord(bytes32,bytes32,address,address,uint64)" \
        "$NODE" "$LABEL_HASH" "$PARENT_OWNER" "$NEW_OR" 0 \
        --from "$PARENT_OWNER" --unlocked \
        --rpc-url "$LOCAL_RPC" --json 2>&1) || {
        FLEET_FAILS=$((FLEET_FAILS + 1))
        echo "$TX_OUT" >> "$SIM_OUTPUT_DIR/step3-fleet.log"
        printf "    ❌ setSubnodeRecord %-20s failed (see step3-fleet.log)\n" "$label"
        continue
    }
    TX_GAS=$(echo "$TX_OUT" | python3 -c "import sys,json; d=json.loads(sys.stdin.read() or '{}'); print(int(d.get('gasUsed','0xC350'),16))" 2>/dev/null || echo "50000")
    STEP3_GAS_TOTAL=$((STEP3_GAS_TOTAL + TX_GAS))
done

if [ "$FLEET_FAILS" -gt 0 ]; then
    echo "  ⚠️  STEP 3 had $FLEET_FAILS / ${#ALL_LABELS[@]} failures (full log: $SIM_OUTPUT_DIR/step3-fleet.log)"
fi

# Use measured total when available; fall back to 3M conservative
# estimate (60 × 50K) if every tx failed and we have nothing.
if [ "$STEP3_GAS_TOTAL" -eq 0 ]; then
    STEP3_GAS=3000000
else
    STEP3_GAS=$STEP3_GAS_TOTAL
fi
echo "  STEP 3 gas total: ${STEP3_GAS} (~ \$$(awk "BEGIN { printf \"%.2f\", $STEP3_GAS * 50 / 1e9 * 4000 }"))"
echo

# ============================================================
# Verify final state
# ============================================================
echo "[4/4] Final state spot-check (5 subnames)"
SPOT_CHECK=(research trader auditor agent-001 agent-050)
ALL_OK=1
for label in "${SPOT_CHECK[@]}"; do
    SUBNODE=$(cast namehash "$label.sbo3lagent.eth")
    SUB_RESOLVER=$(cast call "$ENS_REGISTRY" "resolver(bytes32)(address)" "$SUBNODE" --rpc-url "$LOCAL_RPC")
    if [[ "${SUB_RESOLVER,,}" == "${NEW_OR,,}" ]]; then
        printf "  ✅ %-30s resolver = %s\n" "$label.sbo3lagent.eth" "$SUB_RESOLVER"
    else
        printf "  ❌ %-30s resolver = %s (expected %s)\n" "$label.sbo3lagent.eth" "$SUB_RESOLVER" "$NEW_OR"
        ALL_OK=0
    fi
done
echo

# ============================================================
# Cost summary
# ============================================================
TOTAL_GAS=$((STEP1_GAS + STEP2_GAS + STEP3_GAS))
TOTAL_GAS_BUFFERED=$(( TOTAL_GAS + TOTAL_GAS / 5 ))   # +20% headroom

echo "==================================================================="
echo "  SIMULATION RESULT"
echo "==================================================================="
echo "  STEP 1 OR deploy:           ${STEP1_GAS} gas"
echo "  STEP 2 setResolver:         ${STEP2_GAS} gas"
echo "  STEP 3 60x setSubnodeRecord: ${STEP3_GAS} gas"
echo "  ----------------------------------------------------------------"
echo "  TOTAL:                       ${TOTAL_GAS} gas"
echo "  TOTAL +20% headroom:         ${TOTAL_GAS_BUFFERED} gas"
echo
echo "  At 50 gwei + ETH=\$4000:"
TOTAL_USD=$(awk "BEGIN { printf \"%.2f\", $TOTAL_GAS_BUFFERED * 50 / 1e9 * 4000 }")
echo "    Total: \$${TOTAL_USD}"
echo
echo "  Final on-chain state:"
echo "    sbo3lagent.eth resolver:     $NEW_RESOLVER"
echo "    research.sbo3lagent.eth:     $(cast call "$ENS_REGISTRY" "resolver(bytes32)(address)" "$(cast namehash research.sbo3lagent.eth)" --rpc-url "$LOCAL_RPC")"
echo "    agent-001.sbo3lagent.eth:    $(cast call "$ENS_REGISTRY" "resolver(bytes32)(address)" "$(cast namehash agent-001.sbo3lagent.eth)" --rpc-url "$LOCAL_RPC")"
echo
if [[ "$ALL_OK" == "1" ]]; then
    echo "  ✅ ALL SPOT-CHECKS PASSED — broadcast safely"
    echo "==================================================================="
    exit 0
else
    echo "  ❌ SPOT-CHECKS FAILED — investigate $SIM_OUTPUT_DIR/ logs before broadcasting"
    echo "==================================================================="
    exit 1
fi
