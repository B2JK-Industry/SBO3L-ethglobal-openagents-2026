# R13 P1 + P2 deploy runbook (Daniel-side)

**Status:** **gated on operator funds.** R13 P1 (5-chain
SBO3LReputationRegistry deploys) needs ~$2 testnet gas across 3
testnets; R13 P2 (mainnet OffchainResolver migration) needs ~$10
mainnet. Neither can run from CI; both require Daniel's wallet +
careful order of operations.

This doc captures the runbook so the operator-side execution is
unambiguous when funds are available. **All Rust + Solidity for
both paths is shipped; this doc is the missing connective tissue.**

## P1 — 5-chain SBO3LReputationRegistry deploys

### Targets

| Network | Chain ID | Gas estimate | RPC env var |
|---|---|---|---|
| Sepolia | 11155111 | ~0.001 SEP-ETH (free) | `SEPOLIA_RPC_URL` |
| Optimism Sepolia | 11155420 | ~0.0005 OP-Sepolia ETH | `OPTIMISM_SEPOLIA_RPC_URL` |
| Base Sepolia | 84532 | ~0.0005 Base-Sepolia ETH | `BASE_SEPOLIA_RPC_URL` |
| Arbitrum Sepolia | 421614 | ~0.0001 Arb-Sepolia ETH | `ARBITRUM_SEPOLIA_RPC_URL` |
| Polygon Amoy | 80002 | ~0.001 Amoy MATIC | `POLYGON_AMOY_RPC_URL` |

Note: only Sepolia + Optimism Sepolia + Base Sepolia were named in
R12. R13 adds Arbitrum Sepolia + Polygon Amoy. The deploy script
([`scripts/deploy-reputation-registry.sh`](../../scripts/deploy-reputation-registry.sh))
needs two new entries; see "Script extension" below.

### Pre-flight

```bash
# 1. Wallet — Daniel's existing 0xdc7EFA…D231 with testnet ETH on
#    each of the 5 networks. Bridge from Sepolia via the official
#    bridges if needed:
#      - https://sepolia.optimism.io  (OP Sepolia bridge)
#      - https://sepolia.basescan.org  (Base Sepolia bridge)
#      - https://bridge.arbitrum.io    (Arbitrum Sepolia bridge)
#      - https://faucet.polygon.technology  (Amoy faucet)
# 2. Per-chain RPC URLs from Alchemy / Infura / public RPC.
# 3. PRIVATE_KEY env var = Daniel's deployer key.
```

### Script extension (R13 follow-up to R12 #272)

The R12 deploy wrapper supports {sepolia, optimism-sepolia,
base-sepolia, mainnet, optimism, base}. R13 adds Arbitrum Sepolia
+ Polygon Amoy. Open a tiny PR extending the case statement in
`scripts/deploy-reputation-registry.sh`:

```bash
# Add two cases to the script's NETWORK switch:
arbitrum-sepolia)
  RPC_ENV="ARBITRUM_SEPOLIA_RPC_URL"
  CHAIN_ID=421614
  IS_MAINNET=0
  ;;
polygon-amoy)
  RPC_ENV="POLYGON_AMOY_RPC_URL"
  CHAIN_ID=80002
  IS_MAINNET=0
  ;;
```

And mirror the entries in `crates/sbo3l-cli/src/agent_reputation_multichain.rs`'s
`CHAINS` array (with `registry_addr: None` slots).

### Deploy commands (per chain)

```bash
export PRIVATE_KEY=0x<deployer-secret>

# 1. Sepolia
export SEPOLIA_RPC_URL=https://eth-sepolia.g.alchemy.com/v2/<key>
./scripts/deploy-reputation-registry.sh sepolia

# 2. Optimism Sepolia
export OPTIMISM_SEPOLIA_RPC_URL=https://sepolia.optimism.io
./scripts/deploy-reputation-registry.sh optimism-sepolia

# 3. Base Sepolia
export BASE_SEPOLIA_RPC_URL=https://sepolia.base.org
./scripts/deploy-reputation-registry.sh base-sepolia

# 4. Arbitrum Sepolia (after script extension lands)
export ARBITRUM_SEPOLIA_RPC_URL=https://sepolia-rollup.arbitrum.io/rpc
./scripts/deploy-reputation-registry.sh arbitrum-sepolia

# 5. Polygon Amoy (after script extension lands)
export POLYGON_AMOY_RPC_URL=https://rpc-amoy.polygon.technology
./scripts/deploy-reputation-registry.sh polygon-amoy
```

Each run writes the deployed address to
`deployments/reputation-registry-<network>.txt`.

### Pin the addresses

```bash
# Read the 5 address files
cat deployments/reputation-registry-{sepolia,optimism-sepolia,base-sepolia,arbitrum-sepolia,polygon-amoy}.txt
```

Then edit `crates/sbo3l-cli/src/agent_reputation_multichain.rs`'s
`CHAINS` array, replacing each `registry_addr: None` with the
concrete `Some("0x…")`. Open a follow-up PR for the pinning;
test suite re-runs on the PR; CI green = deploys verified.

### Run multi-chain broadcast

After all 5 addresses pinned + PR landed:

```bash
export SBO3L_SIGNER_KEY="$PRIVATE_KEY"
export SBO3L_RPC_URL_SEPOLIA="$SEPOLIA_RPC_URL"
# … same for the other 4 chains

sbo3l agent reputation-publish \
  --fqdn research-agent.sbo3lagent.eth \
  --events events.json \
  --multi-chain sepolia,optimism-sepolia,base-sepolia,arbitrum-sepolia,polygon-amoy
```

Output: 5 per-chain tx hashes + Etherscan links. Cross-chain
consistency verified via `sbo3l agent reputation-aggregate` (R12
#272) consuming the snapshot of all 5 chains.

### Cost ceiling (R13 P1)

| Network | One-time deploy | Per-publish |
|---|---|---|
| Sepolia | ~0.001 SEP-ETH | ~50k gas (free) |
| Optimism Sepolia | ~0.0005 OP-Sep | ~30k gas (free) |
| Base Sepolia | ~0.0005 Base-Sep | ~30k gas (free) |
| Arbitrum Sepolia | ~0.0001 Arb-Sep | ~20k gas (free) |
| Polygon Amoy | ~0.001 Amoy MATIC | ~50k gas (free) |

Total testnet cost: under $2 across all 5 chains.

## P2 — Mainnet OffchainResolver migration

### Decision posture

R12's
[`docs/design/mainnet-deploy-decision.md`](../design/mainnet-deploy-decision.md)
documented the explicit **SKIP** of mainnet OffchainResolver
migration for the hackathon submission. R13 P2 reopens the
decision: if Daniel commits ~$10 mainnet gas, we proceed.

The R12 doc lists three "conditions to revisit." Verify **at
least one** has triggered before proceeding:

1. External partner asked for OffchainResolver-served records?
2. Reputation publisher needs faster-than-`setText` updates?
3. ENS / third-party reviewer raised mainnet specifically?

If none triggered, the SKIP decision still stands. Document the
revisit reason in a follow-up commit to the decision doc before
running the deploy.

### Pre-flight

```bash
# 1. Mainnet RPC + Daniel's mainnet-funded wallet (~$10 ETH).
# 2. Fresh gateway-signing key — DO NOT reuse the Sepolia signer
#    (cross-network key reuse expands the blast radius).
# 3. Migration plan agreed: which records move to the new resolver
#    in which order. The 5 existing records on sbo3lagent.eth are
#    the migration target.
# 4. Rollback plan: setResolver back to the previous resolver if
#    the new OffchainResolver misbehaves in production.
```

### Deploy commands

```bash
export PRIVATE_KEY=0x<mainnet-deployer-secret>
export SBO3L_ALLOW_MAINNET_TX=1
export MAINNET_RPC_URL=https://eth-mainnet.g.alchemy.com/v2/<key>
export GATEWAY_SIGNER=0x<fresh-mainnet-gateway-signer-address>

cd crates/sbo3l-identity/contracts
forge create OffchainResolver.sol:OffchainResolver \
  --rpc-url $MAINNET_RPC_URL \
  --private-key $PRIVATE_KEY \
  --constructor-args "$GATEWAY_SIGNER" '["https://sbo3l-ccip.vercel.app/api/{sender}/{data}.json"]' \
  --verify
```

Pin the deployed mainnet address in
`crates/sbo3l-identity/src/contracts.rs`'s
`OFFCHAIN_RESOLVER_MAINNET` constant (currently absent — R9 P1
shipped the Sepolia pin only).

### Migration

The mainnet apex `sbo3lagent.eth` currently has 5 records on the
canonical PublicResolver
(`0xF29100983E058B709F3D539b0c765937B804AC15`). Migration:

```bash
# 1. Re-publish the 5 records on the gateway side. The gateway must
#    serve them BEFORE setResolver flips, otherwise resolution
#    breaks for the migration window.

# 2. Rehearse the swap on Sepolia first. The Sepolia OffchainResolver
#    is at 0x7c6913…aCA8c3. Verify the migration flow end-to-end
#    against a Sepolia subname before doing mainnet.

# 3. Switch the mainnet apex resolver:
NODE=$(cast namehash sbo3lagent.eth)
NEW_RESOLVER=$(cat deployments/offchain-resolver-mainnet.txt)

cast send 0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e \
  "setResolver(bytes32,address)" \
  "$NODE" "$NEW_RESOLVER" \
  --rpc-url $MAINNET_RPC_URL \
  --private-key $PRIVATE_KEY

# 4. Verify resolution:
cast call $NEW_RESOLVER \
  "resolve(bytes,bytes)" \
  "$(cast --to-bytes32 'sbo3lagent.eth')" \
  "$(cast calldata 'text(bytes32,string)' $NODE 'sbo3l:agent_id')" \
  --rpc-url $MAINNET_RPC_URL
# Expect: OffchainLookup revert with the gateway URL

# 5. End-to-end via viem (same as the Sepolia E2E from #232):
cd examples/t-4-1-viem-e2e
SBO3L_OFFCHAIN_RESOLVER=$NEW_RESOLVER \
SBO3L_SEPOLIA_RPC_URL=$MAINNET_RPC_URL \
  pnpm start sbo3lagent.eth sbo3l:agent_id
```

### Rollback (if migration misbehaves)

```bash
# The previous resolver address is the canonical PublicResolver:
PREVIOUS_RESOLVER=0xF29100983E058B709F3D539b0c765937B804AC15

cast send 0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e \
  "setResolver(bytes32,address)" \
  "$NODE" "$PREVIOUS_RESOLVER" \
  --rpc-url $MAINNET_RPC_URL \
  --private-key $PRIVATE_KEY

# The 5 records pre-existed on the canonical resolver; rollback
# restores them. Any post-migration record updates are lost.
```

## Cost summary

| Component | Cost ceiling |
|---|---|
| 5-chain testnet deploys (P1) | < $2 |
| Mainnet OffchainResolver deploy + migration (P2) | ~$10 |
| **Total** | **~$12** |

P1 is unambiguously a go (testnet costs are nominal). P2 stays
gated until one of the R12 SKIP-decision revisit conditions
triggers.

## Why this doc instead of running the deploys

R13 explicitly named "Daniel provides funds (~$2 total testnet)"
and "If Daniel commits ~$10" as gates. Without the wallet +
private key, no agent can run these deploys. This runbook captures
the precise unblock steps so the moment funds + key are available,
execution is one shell-session.

The Rust + Solidity surface is fully shipped:

- [`crates/sbo3l-identity/contracts/SBO3LReputationRegistry.sol`](../../crates/sbo3l-identity/contracts/SBO3LReputationRegistry.sol) — R11 P1
- [`crates/sbo3l-identity/contracts/script/DeployReputationRegistry.s.sol`](../../crates/sbo3l-identity/contracts/script/DeployReputationRegistry.s.sol) — R11 P1
- [`scripts/deploy-reputation-registry.sh`](../../scripts/deploy-reputation-registry.sh) — R12
- [`crates/sbo3l-cli/src/agent_reputation_multichain.rs`](../../crates/sbo3l-cli/src/agent_reputation_multichain.rs) — R11 P2 + R12

Daniel's responsibility from here: wallet + RPC URLs + execute.
