#!/usr/bin/env bash
# Deploy SBO3LReputationRegistry to one chain.
#
# Wraps the Foundry deploy script
# (crates/sbo3l-identity/contracts/script/DeployReputationRegistry.s.sol)
# with the conventions the rest of the SBO3L scripts use:
#
#  * Per-chain operator-level config via env vars (RPC URL,
#    private key, optional Etherscan API key for verification).
#  * Mainnet double-gate: refuses --network mainnet without
#    SBO3L_ALLOW_MAINNET_TX=1.
#  * Idempotent address capture: writes the deployed address to
#    deployments/reputation-registry-${NETWORK}.txt so subsequent
#    operations can read it without re-deploying.
#
# Usage:
#   ./scripts/deploy-reputation-registry.sh <network>
#
# Where <network> is one of:
#   sepolia | optimism-sepolia | base-sepolia | mainnet | optimism | base
#
# Required env vars (per network):
#   <NETWORK>_RPC_URL        e.g. SEPOLIA_RPC_URL
#   PRIVATE_KEY              0x-prefixed 32-byte hex
#
# Optional:
#   <NETWORK>_ETHERSCAN_API_KEY  passed to forge verify-contract
#   SBO3L_ALLOW_MAINNET_TX=1     required for any mainnet target
#
# Output:
#   deployments/reputation-registry-<network>.txt
#     One line, the 0x-prefixed deployment address.
#
# After deploy: pin the address in
#   crates/sbo3l-cli/src/agent_reputation_multichain.rs
#     under the corresponding ChainSpec entry.

set -euo pipefail

NETWORK="${1:-}"
if [[ -z "$NETWORK" ]]; then
  echo "usage: $0 <network>" >&2
  echo "  networks: sepolia, optimism-sepolia, base-sepolia, mainnet, optimism, base" >&2
  exit 2
fi

# Map the network label to (RPC env var name, chain_id, is_mainnet).
case "$NETWORK" in
  sepolia)
    RPC_ENV="SEPOLIA_RPC_URL"
    CHAIN_ID=11155111
    IS_MAINNET=0
    ;;
  optimism-sepolia)
    RPC_ENV="OPTIMISM_SEPOLIA_RPC_URL"
    CHAIN_ID=11155420
    IS_MAINNET=0
    ;;
  base-sepolia)
    RPC_ENV="BASE_SEPOLIA_RPC_URL"
    CHAIN_ID=84532
    IS_MAINNET=0
    ;;
  mainnet)
    RPC_ENV="MAINNET_RPC_URL"
    CHAIN_ID=1
    IS_MAINNET=1
    ;;
  optimism)
    RPC_ENV="OPTIMISM_RPC_URL"
    CHAIN_ID=10
    IS_MAINNET=1
    ;;
  base)
    RPC_ENV="BASE_RPC_URL"
    CHAIN_ID=8453
    IS_MAINNET=1
    ;;
  *)
    echo "ERROR: unknown network: $NETWORK" >&2
    echo "  expected one of: sepolia, optimism-sepolia, base-sepolia, mainnet, optimism, base" >&2
    exit 2
    ;;
esac

# Mainnet double-gate. Same convention agent register / reputation
# broadcast use elsewhere in the codebase.
if [[ "$IS_MAINNET" -eq 1 ]] && [[ "${SBO3L_ALLOW_MAINNET_TX:-}" != "1" ]]; then
  cat >&2 <<'EOF'
ERROR: refusing mainnet deploy without SBO3L_ALLOW_MAINNET_TX=1.

Mainnet deploys cost real gas (~$3-10 at 50 gwei). Set the env
var to acknowledge before re-running:

    export SBO3L_ALLOW_MAINNET_TX=1
    ./scripts/deploy-reputation-registry.sh mainnet
EOF
  exit 2
fi

# Required env vars.
RPC_URL="${!RPC_ENV:-}"
if [[ -z "$RPC_URL" ]]; then
  echo "ERROR: $RPC_ENV not set; need an RPC URL for $NETWORK" >&2
  exit 2
fi
if [[ -z "${PRIVATE_KEY:-}" ]]; then
  echo "ERROR: PRIVATE_KEY not set; need 0x-prefixed 32-byte hex deployer key" >&2
  exit 2
fi

# Pinned working dir for forge.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CONTRACTS_DIR="$REPO_ROOT/crates/sbo3l-identity/contracts"

# Make sure forge-std is present (one-time setup; no-op if already there).
if [[ ! -d "$CONTRACTS_DIR/lib/forge-std" ]]; then
  echo "==> installing forge-std (one-time)"
  (cd "$CONTRACTS_DIR" && forge install foundry-rs/forge-std --no-git)
fi

DEPLOYMENTS_DIR="$REPO_ROOT/deployments"
mkdir -p "$DEPLOYMENTS_DIR"
ADDR_FILE="$DEPLOYMENTS_DIR/reputation-registry-${NETWORK}.txt"

echo "==> deploying SBO3LReputationRegistry"
echo "    network:   $NETWORK"
echo "    chain_id:  $CHAIN_ID"
echo "    rpc:       $(echo "$RPC_URL" | sed -E 's|^(https?://[^/]+).*|\1|')/<redacted>"
echo "    addr_out:  $ADDR_FILE"
echo

# forge script invocation. `--broadcast` sends the tx; `--json`
# emits machine-readable output that we parse for the deployed
# address.
DEPLOY_OUTPUT="$(
  cd "$CONTRACTS_DIR" && \
  forge script script/DeployReputationRegistry.s.sol \
    --rpc-url "$RPC_URL" \
    --broadcast \
    --json \
    --quiet \
    2>&1
)" || {
  echo "$DEPLOY_OUTPUT" >&2
  echo "ERROR: forge script failed" >&2
  exit 1
}

# Parse the deployed address. forge emits a "Logs" array with the
# console.log() output from the script; the script's last log is
# "SBO3LReputationRegistry deployed to: <addr>". Extract via grep
# rather than jq — we don't want a hard jq dep on the deploy host.
DEPLOYED_ADDR="$(
  echo "$DEPLOY_OUTPUT" \
    | grep -oE '"0x[0-9a-fA-F]{40}"' \
    | head -1 \
    | tr -d '"'
)"

if [[ -z "$DEPLOYED_ADDR" ]]; then
  echo "ERROR: could not parse deployed address from forge output" >&2
  echo "--- forge output ---" >&2
  echo "$DEPLOY_OUTPUT" >&2
  exit 1
fi

echo "$DEPLOYED_ADDR" > "$ADDR_FILE"
echo "==> deployed at: $DEPLOYED_ADDR"
echo "==> address pinned: $ADDR_FILE"
echo

# Optional verification on Etherscan-compatible explorers.
ETHERSCAN_ENV="$(echo "$NETWORK" | tr 'a-z-' 'A-Z_')_ETHERSCAN_API_KEY"
ETHERSCAN_API_KEY="${!ETHERSCAN_ENV:-}"
if [[ -n "$ETHERSCAN_API_KEY" ]]; then
  echo "==> verifying on Etherscan ($ETHERSCAN_ENV present)"
  (cd "$CONTRACTS_DIR" && \
    forge verify-contract \
      --chain-id "$CHAIN_ID" \
      --etherscan-api-key "$ETHERSCAN_API_KEY" \
      "$DEPLOYED_ADDR" \
      SBO3LReputationRegistry \
  ) || {
    echo "WARNING: verification failed (deploy still succeeded)" >&2
  }
else
  echo "==> skip verification ($ETHERSCAN_ENV not set)"
fi

echo
echo "Next step: pin this address in"
echo "  crates/sbo3l-cli/src/agent_reputation_multichain.rs"
echo "  under the ChainSpec for '$NETWORK' (registry_addr field)."
