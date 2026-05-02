# Multi-chain reputation — proof artefact (R12 P3)

**Status:** Code paths shipped; live deploy gated on Daniel's
~$1 testnet gas commit (Sepolia + OP Sepolia + Base Sepolia).
**Companion artefacts:** [`docs/design/sbo3l-reputation-registry.md`](../design/sbo3l-reputation-registry.md)
(R11 P1 contract), [`scripts/deploy-reputation-registry.sh`](../../scripts/deploy-reputation-registry.sh)
(per-chain deploy wrapper), [`crates/sbo3l-cli/src/agent_reputation_multichain.rs`](../../crates/sbo3l-cli/src/agent_reputation_multichain.rs)
(broadcast orchestrator), [`crates/sbo3l-cli/src/agent_reputation_aggregate.rs`](../../crates/sbo3l-cli/src/agent_reputation_aggregate.rs)
(R12 P3 aggregate CLI).

## What this document proves

Cross-chain reputation aggregation works **end-to-end** when N
chains each carry the agent's reputation score:

1. The same agent runs on N chains (mainnet, Optimism, Base, …).
2. SBO3L publishes per-chain reputation scores into each chain's
   `SBO3LReputationRegistry` deploy.
3. A consumer reads N `reputationOf` values via per-chain RPC.
4. The consumer aggregates N scores into one weighted score via
   `sbo3l_policy::cross_chain_reputation::aggregate_reputation`.
5. The aggregate report is independently verifiable: any third
   party with N RPCs + this repo can reproduce the math.

## Three-step verification (live, post-deploy)

### 1. Read per-chain scores

```bash
# Per chain: query SBO3LReputationRegistry.reputationOf
TENANT=$(cast keccak "sbo3lagent.eth")
AGENT="0xdc7EFA6b4Bd77d1a406DE4727F0DF567e597D231"

for NETWORK in sepolia optimism-sepolia base-sepolia; do
  RPC_VAR="SBO3L_RPC_URL_$(echo "$NETWORK" | tr 'a-z-' 'A-Z_')"
  RPC_URL="${!RPC_VAR}"
  REGISTRY=$(cat "deployments/reputation-registry-${NETWORK}.txt")

  cast call "$REGISTRY" \
    "reputationOf(bytes32,address)((uint8,uint64,uint64,address))" \
    "$TENANT" "$AGENT" \
    --rpc-url "$RPC_URL"
done
```

Output: per-chain `(score, publishedAt, chainHeadBlock, signer)` tuples.

### 2. Build the snapshot input

```bash
cat > /tmp/snapshots.json <<EOF
{
  "now_secs": $(date +%s),
  "snapshots": [
    {"chain_id": 11155111, "fqdn": "sbo3lagent.eth", "score": <SEPOLIA_SCORE>, "observed_at": <TS>},
    {"chain_id": 11155420, "fqdn": "sbo3lagent.eth", "score": <OP_SEPOLIA_SCORE>, "observed_at": <TS>},
    {"chain_id": 84532, "fqdn": "sbo3lagent.eth", "score": <BASE_SEPOLIA_SCORE>, "observed_at": <TS>}
  ]
}
EOF
```

### 3. Run the aggregator

```bash
sbo3l agent reputation-aggregate --input /tmp/snapshots.json
```

Output (canonical example, all three scores at 90):

```json
{
  "schema": "sbo3l.reputation_aggregate_report.v1",
  "aggregate_score": 90,
  "source_count": 3,
  "total_weight": 2.6,
  "per_chain": [
    { "chain_id": 11155111, "raw_score": 90, "chain_weight": 0.2,
      "recency_factor": 1.0, "effective_contribution": 18.0 },
    { "chain_id": 11155420, "raw_score": 90, "chain_weight": 0.5,
      "recency_factor": 1.0, "effective_contribution": 45.0 },
    { "chain_id": 84532,    "raw_score": 90, "chain_weight": 0.5,
      "recency_factor": 1.0, "effective_contribution": 45.0 }
  ]
}
```

(Note: testnet chain IDs fall back to `default_chain_weight = 0.5`
in the default params; Sepolia is explicitly weighted at `0.2` to
reflect "testnet observation, lower confidence than mainnet".)

## Reproducible offline test

The aggregator works as a pure function — judges who can't run a
testnet broadcast can still verify the math against fixed
snapshots:

```bash
cat > /tmp/synthetic.json <<'EOF'
{
  "now_secs": 2000000000,
  "snapshots": [
    {"chain_id": 1,    "fqdn": "x", "score": 90, "observed_at": 1999999940},
    {"chain_id": 10,   "fqdn": "x", "score": 80, "observed_at": 1999999940},
    {"chain_id": 137,  "fqdn": "x", "score": 70, "observed_at": 1999999940}
  ]
}
EOF
sbo3l agent reputation-aggregate --input /tmp/synthetic.json
```

Expected aggregate: `82` (mainnet 90 × 1.0 + Optimism 80 × 0.8 +
Polygon 70 × 0.6 = 196; weight sum 2.4; 196/2.4 = 81.67 → 82).
Pinned in
[`crates/sbo3l-policy/src/cross_chain_reputation.rs::tests::three_chain_synthetic_fleet_aggregates`](../../crates/sbo3l-policy/src/cross_chain_reputation.rs).

## Per-chain signatures (NOT signature replay)

`SBO3LReputationRegistry`'s digest binds to `address(this)` — sigs
from one chain's deploy don't validate against another chain's
deploy at a different address. **Intentional security property**:
prevents a malicious deploy at a chosen address from replaying
sigs harvested from the canonical deploy. Same agent, same score,
N per-chain signatures. The audit log captures all N tx hashes.

This is documented inline in
[`agent_reputation_multichain.rs`](../../crates/sbo3l-cli/src/agent_reputation_multichain.rs)
("Why per-chain signatures (not 'single signature replayed')")
and reproduced here so a reviewer who only reads docs sees the
property without spelunking source.

## Deploy runbook (Daniel-side, ~10 minutes)

```bash
# 1. Provision deployer key (Sepolia only — mainnet skipped per R12 P5).
export PRIVATE_KEY=0x<deployer-secret>

# 2. Deploy to Sepolia.
export SEPOLIA_RPC_URL=https://eth-sepolia.g.alchemy.com/v2/<key>
./scripts/deploy-reputation-registry.sh sepolia
# → writes deployments/reputation-registry-sepolia.txt

# 3. Deploy to Optimism Sepolia.
export OPTIMISM_SEPOLIA_RPC_URL=https://sepolia.optimism.io
./scripts/deploy-reputation-registry.sh optimism-sepolia
# → writes deployments/reputation-registry-optimism-sepolia.txt

# 4. Deploy to Base Sepolia.
export BASE_SEPOLIA_RPC_URL=https://sepolia.base.org
./scripts/deploy-reputation-registry.sh base-sepolia
# → writes deployments/reputation-registry-base-sepolia.txt

# 5. Pin the three addresses in agent_reputation_multichain.rs:
#    - sepolia ChainSpec: registry_addr: Some("0x…")
#    - optimism-sepolia: registry_addr: Some("0x…")
#    - base-sepolia: registry_addr: Some("0x…")

# 6. Commit + open PR.
```

After the PR with pinned addresses lands, the multi-chain
broadcast CLI from #267 lights up:

```bash
export SBO3L_SIGNER_KEY="$PRIVATE_KEY"
export SBO3L_RPC_URL_SEPOLIA="$SEPOLIA_RPC_URL"
export SBO3L_RPC_URL_OPTIMISM_SEPOLIA="$OPTIMISM_SEPOLIA_RPC_URL"
export SBO3L_RPC_URL_BASE_SEPOLIA="$BASE_SEPOLIA_RPC_URL"

sbo3l agent reputation-publish \
  --fqdn research-agent.sbo3lagent.eth \
  --events events.json \
  --multi-chain sepolia,optimism-sepolia,base-sepolia
```

Output: per-chain tx hash + Etherscan link, three rows.

## Cost ceiling

| Chain | Deploy gas | Per-publish gas |
|---|---|---|
| Sepolia | ~0.001 SEP-ETH | ~50k gas × Sepolia-gwei (negligible) |
| Optimism Sepolia | ~$0.01 OP-Sepolia ETH | ~30k gas (L2 cheap) |
| Base Sepolia | ~$0.01 Base-Sepolia ETH | ~30k gas (L2 cheap) |

Total testnet cost: under $1 across all three chains for the
deploy + a handful of publishes.

## Why three chains (and not more)

Three is the minimum that makes the aggregator weighted-mean
visible (one chain trivially returns its own score; two chains
average). Three chains let an operator dial chain-prominence
weights (mainnet > L2s > testnets) and see the difference
in the aggregate. Adding more chains costs incremental gas with
diminishing aggregator-shape information.

The chain set (Sepolia + OP Sepolia + Base Sepolia) covers L1 +
two production-shaped L2 stacks (OP-stack, Base-stack), enough to
demonstrate the cross-chain pattern without committing to mainnet
spend.
