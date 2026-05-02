#!/usr/bin/env bash
#
# Deploy SBO3L's OffchainResolver to Sepolia (or mainnet, with the
# explicit double-gate). Mirrors `sbo3l agent register`'s
# SBO3L_ALLOW_MAINNET_TX gate.
#
# What this does:
#   1. Reads GATEWAY_SIGNER_ADDRESS (the address corresponding to the
#      gateway's GATEWAY_PRIVATE_KEY in Vercel env).
#   2. Reads SEPOLIA_RPC_URL (or MAINNET_RPC_URL with the gate set).
#   3. Reads SBO3L_DEPLOYER_PRIVATE_KEY (Daniel's wallet, dev/test
#      key — never reused with a funded mainnet wallet).
#   4. forge create OffchainResolver(gatewaySigner, urls).
#   5. Prints the deployed address + the Etherscan link.
#
# Daniel-runnable end-to-end in ~3-4 minutes:
#   export GATEWAY_SIGNER_ADDRESS=0x...               # match Vercel env
#   export SEPOLIA_RPC_URL=https://sepolia...         # Alchemy endpoint
#   export SBO3L_DEPLOYER_PRIVATE_KEY=0x...           # dev key, ~$3 ETH
#   ./scripts/deploy-offchain-resolver.sh
#
# After deploy:
#   cast send <ENS Registry> "setResolver(bytes32,address)" \
#     $(cast call <ENS Registry> "0x... namehash sbo3lagent.eth")
#     <DEPLOYED_ADDRESS> --rpc-url $RPC --private-key $KEY
#
# See docs/design/T-4-1-offchain-resolver-deploy.md for full
# step-by-step.

set -euo pipefail

cd "$(dirname "$0")/.."

CONTRACTS_DIR="crates/sbo3l-identity/contracts"
GATEWAY_URL_TEMPLATE="${GATEWAY_URL_TEMPLATE:-https://sbo3l-ccip.vercel.app/api/{sender}/{data}.json}"
NETWORK="${NETWORK:-sepolia}"

require_env() {
    if [ -z "${!1:-}" ]; then
        echo "ERROR: $1 is required but not set." >&2
        echo "       See scripts/deploy-offchain-resolver.sh head comment." >&2
        exit 2
    fi
}

require_env GATEWAY_SIGNER_ADDRESS
require_env SBO3L_DEPLOYER_PRIVATE_KEY

case "$NETWORK" in
    sepolia)
        require_env SEPOLIA_RPC_URL
        RPC_URL="$SEPOLIA_RPC_URL"
        ETHERSCAN_BASE="https://sepolia.etherscan.io/address"
        ;;
    mainnet)
        if [ "${SBO3L_ALLOW_MAINNET_TX:-}" != "1" ]; then
            cat >&2 <<EOF
ERROR: refusing --network mainnet without SBO3L_ALLOW_MAINNET_TX=1.

Mainnet OffchainResolver deployment is gas-bearing (~\$5-10 at 50 gwei,
plus the per-name `setResolver` op afterwards). Set
SBO3L_ALLOW_MAINNET_TX=1 to acknowledge before re-running.

The default network is sepolia; flip via NETWORK=sepolia (default) or
NETWORK=mainnet.
EOF
            exit 2
        fi
        require_env MAINNET_RPC_URL
        RPC_URL="$MAINNET_RPC_URL"
        ETHERSCAN_BASE="https://etherscan.io/address"
        ;;
    *)
        echo "ERROR: NETWORK must be \`sepolia\` or \`mainnet\`; got \`$NETWORK\`." >&2
        exit 2
        ;;
esac

# Validate that GATEWAY_SIGNER_ADDRESS is 0x + 40 hex.
if ! [[ "$GATEWAY_SIGNER_ADDRESS" =~ ^0x[0-9a-fA-F]{40}$ ]]; then
    echo "ERROR: GATEWAY_SIGNER_ADDRESS must be 0x + 40 hex chars; got \`$GATEWAY_SIGNER_ADDRESS\`." >&2
    exit 2
fi

echo "==> Network:                $NETWORK"
echo "==> Gateway URL template:   $GATEWAY_URL_TEMPLATE"
echo "==> Gateway signer address: $GATEWAY_SIGNER_ADDRESS"
echo

# Make sure forge-std is installed; install lazily if not.
if [ ! -d "$CONTRACTS_DIR/lib/forge-std" ]; then
    echo "==> forge-std not found, installing v1.10.0 ..."
    (cd "$CONTRACTS_DIR" && forge install foundry-rs/forge-std@v1.10.0 --no-commit)
fi

echo "==> forge build"
(cd "$CONTRACTS_DIR" && forge build)

echo
echo "==> forge test (unit tests against the mock signer)"
(cd "$CONTRACTS_DIR" && forge test)

echo
echo "==> forge create OffchainResolver on $NETWORK"

DEPLOY_OUTPUT=$(cd "$CONTRACTS_DIR" && forge create \
    OffchainResolver.sol:OffchainResolver \
    --rpc-url "$RPC_URL" \
    --private-key "$SBO3L_DEPLOYER_PRIVATE_KEY" \
    --broadcast \
    --constructor-args "$GATEWAY_SIGNER_ADDRESS" "[$GATEWAY_URL_TEMPLATE]")

echo "$DEPLOY_OUTPUT"

DEPLOYED_ADDRESS=$(echo "$DEPLOY_OUTPUT" | awk '/Deployed to:/ {print $3}')

if [ -z "$DEPLOYED_ADDRESS" ]; then
    echo "ERROR: could not parse deployed address from forge output." >&2
    exit 1
fi

echo
echo "==================================================================="
echo "  DEPLOYED:  $DEPLOYED_ADDRESS"
echo "  ETHERSCAN: $ETHERSCAN_BASE/$DEPLOYED_ADDRESS"
echo "==================================================================="
echo
echo "Next steps (Daniel runs these):"
echo
echo "  1. Pin the address in docs/design/T-4-1-offchain-resolver-deploy.md"
echo "     (replace TBD with the address printed above)."
echo
echo "  2. Set the resolver on sbo3lagent.eth so subnames inherit it:"
echo
echo "     cast send 0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e \\"
echo "       \"setResolver(bytes32,address)\" \\"
echo "       \$(cast namehash sbo3lagent.eth) \\"
echo "       $DEPLOYED_ADDRESS \\"
echo "       --rpc-url \$RPC_URL --private-key \$SBO3L_DEPLOYER_PRIVATE_KEY"
echo
echo "  3. Verify via viem (or any ENSIP-10 client):"
echo
echo "     viem.getEnsText({"
echo "       name: 'research-agent.sbo3lagent.eth',"
echo "       key:  'sbo3l:agent_id',"
echo "     })"
echo "     // expect: 'research-agent-01' (per apps/ccip-gateway/data/records.json)"
echo
