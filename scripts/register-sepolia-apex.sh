#!/usr/bin/env bash
#
# Register `sbo3lagent.eth` on Sepolia via the V3 ETHRegistrarController
# at 0xfb3cE5D01e0f33f41DbB39035dB9745962F1f968, then issue
# `research-agent.sbo3lagent.eth` subname pointing at the
# OffchainResolver redeployed in Task A.
#
# Heidi UAT bug #2 closeout — Task B. Splits the commit-reveal flow
# across two cast invocations with a real bash sleep so forge's
# simulation-time-warp pitfall doesn't apply.
#
# Inputs (env):
#   PRIVATE_KEY                  0x<deployer 32-byte>
#   SEPOLIA_RPC_URL              Alchemy/Infura/PublicNode Sepolia RPC
#   SEPOLIA_OFFCHAIN_RESOLVER    0x<40-hex>; defaults to
#                                0x87e99508C222c6E419734CACbb6781b8d282b1F6
#                                (the Task A redeploy)
#
# Outputs:
#   stdout: tx hashes for commit, register, setSubnodeRecord
#   the calling shell can verify via `cast call ENS_REGISTRY owner(node)` post-run.

set -euo pipefail

if [[ -z "${PRIVATE_KEY:-}" ]]; then
  echo "ERROR: PRIVATE_KEY env var required" >&2
  exit 2
fi
if [[ -z "${SEPOLIA_RPC_URL:-}" ]]; then
  echo "ERROR: SEPOLIA_RPC_URL env var required" >&2
  exit 2
fi

CTRL=0xfb3cE5D01e0f33f41DbB39035dB9745962F1f968
ENS_REGISTRY=0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e
APEX_RESOLVER=0x8FADE66B79cC9f707aB26799354482EB93a5B7dD     # Sepolia PublicResolver
OFFCHAIN_RESOLVER="${SEPOLIA_OFFCHAIN_RESOLVER:-0x87e99508C222c6E419734CACbb6781b8d282b1F6}"

OWNER=$(cast wallet address --private-key "$PRIVATE_KEY")
APEX_LABEL="sbo3lagent"
SUBNAME_LABEL="research-agent"
DURATION=31536000

# Deterministic secret. Same across re-runs so a half-finished commit
# can be resumed (within the 24h maxCommitmentAge window). Bump the
# inner string to invalidate.
SECRET=$(cast keccak "sbo3l-sepolia-apex-2026-05-03")

REQ_TUPLE="(\"$APEX_LABEL\",$OWNER,$DURATION,$SECRET,$APEX_RESOLVER,[],0,0x0000000000000000000000000000000000000000000000000000000000000000)"

echo "==> 1/4 compute commitment via makeCommitment(struct)"
COMMITMENT=$(cast call "$CTRL" \
  "makeCommitment((string,address,uint256,bytes32,address,bytes[],uint8,bytes32))(bytes32)" \
  "$REQ_TUPLE" \
  --rpc-url "$SEPOLIA_RPC_URL")
echo "    commitment: $COMMITMENT"

# If this commitment is already in the controller and ≥minCommitmentAge,
# skip the commit + sleep entirely.
EXISTING=$(cast call "$CTRL" "commitments(bytes32)(uint256)" "$COMMITMENT" --rpc-url "$SEPOLIA_RPC_URL")
if [[ "$EXISTING" == "0" ]]; then
  echo "==> 2/4 send commit($COMMITMENT)"
  cast send "$CTRL" "commit(bytes32)" "$COMMITMENT" \
    --rpc-url "$SEPOLIA_RPC_URL" \
    --private-key "$PRIVATE_KEY" >/dev/null
  echo "    commit landed."

  echo "==> 3/4 wait minCommitmentAge=60 + 10s buffer"
  sleep 70
else
  echo "==> 2-3/4 commitment already on chain at timestamp $EXISTING — skipping commit"
fi

echo "==> 4a/4 read rentPrice and register with 10% buffer"
PRICE_OUT=$(cast call "$CTRL" "rentPrice(string,uint256)((uint256,uint256))" "$APEX_LABEL" "$DURATION" --rpc-url "$SEPOLIA_RPC_URL")
# PRICE_OUT format: "(<base>, <premium>)"
BASE=$(echo "$PRICE_OUT" | sed -E 's/^\(([0-9]+),.*$/\1/')
PREMIUM=$(echo "$PRICE_OUT" | sed -E 's/^\([0-9]+, ([0-9]+)\)$/\1/')
RENT=$((BASE + PREMIUM))
SEND=$(( RENT + RENT / 10 ))
echo "    rent (base+premium) wei: $RENT"
echo "    send value (10% buffer):  $SEND"

echo "==> 4b/4 send register(struct) with msg.value=$SEND"
cast send "$CTRL" \
  "register((string,address,uint256,bytes32,address,bytes[],uint8,bytes32))" \
  "$REQ_TUPLE" \
  --value "$SEND" \
  --rpc-url "$SEPOLIA_RPC_URL" \
  --private-key "$PRIVATE_KEY" >/dev/null
echo "    register landed."

APEX_NODE=$(cast namehash "$APEX_LABEL.eth")
SUB_LABEL_HASH=$(cast keccak "$SUBNAME_LABEL")
SUBNAME_NODE=$(cast keccak "${APEX_NODE}${SUB_LABEL_HASH:2}")

echo "==> issuing subname research-agent.sbo3lagent.eth"
echo "    apex node:    $APEX_NODE"
echo "    sub label:    $SUB_LABEL_HASH"
echo "    subname node: $SUBNAME_NODE"
echo "    resolver:     $OFFCHAIN_RESOLVER (OffchainResolver)"
cast send "$ENS_REGISTRY" \
  "setSubnodeRecord(bytes32,bytes32,address,address,uint64)" \
  "$APEX_NODE" "$SUB_LABEL_HASH" "$OWNER" "$OFFCHAIN_RESOLVER" 0 \
  --rpc-url "$SEPOLIA_RPC_URL" \
  --private-key "$PRIVATE_KEY" >/dev/null
echo "    setSubnodeRecord landed."

echo
echo "==================================================================="
echo "  DONE — research-agent.sbo3lagent.eth on Sepolia points at"
echo "          OffchainResolver $OFFCHAIN_RESOLVER"
echo "==================================================================="
echo
echo "Verify via viem (or any ENSIP-10 client):"
echo "  viem.getEnsText({ name: 'research-agent.sbo3lagent.eth',"
echo "                    key:  'sbo3l:agent_id', chain: sepolia })"
echo "  // expect: 'research-agent-01' (gateway records.json)"
