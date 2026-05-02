# `sbo3l agent reputation-publish --broadcast` — T-4-7

**Status:** Shipped (this PR).
**Depends on:** F-5 EthSigner (#243).
**Companion:** [`sbo3l agent register --broadcast`](agent.md) — same
alloy harness, same env-var conventions, same Etherscan-link
emission shape.

## What it does

Computes an agent's v2 reputation score from an audit-event input
file, builds a `setText("sbo3l:reputation_score", "<score>")` tx
against the agent's ENS resolver, signs it via the same private-key
env-var convention T-3-1 broadcast uses, and sends + confirms the
tx via JSON-RPC.

Single-tx flow. The reputation record is just a setText — no
subname issuance, no multicall.

## Run

### Dry-run (default, no Cargo features needed)

```bash
sbo3l agent reputation-publish \
  --fqdn research-agent.sbo3lagent.eth \
  --events events.json \
  --network sepolia
```

Prints the envelope JSON (calldata, namehash, score, resolver). No
network calls, no signing.

### Broadcast (Sepolia)

```bash
# Build with the feature (one-time)
cargo install sbo3l-cli --features eth_broadcast

# Then:
export SBO3L_RPC_URL='https://eth-sepolia.g.alchemy.com/v2/<key>'
export SBO3L_SIGNER_KEY='<32-byte hex private key>'

sbo3l agent reputation-publish \
  --fqdn research-agent.sbo3lagent.eth \
  --events events.json \
  --network sepolia \
  --broadcast
```

Prints the score, sends the `setText` tx, waits one confirmation,
prints the tx hash + Etherscan link + gas used.

### Broadcast (mainnet)

```bash
export SBO3L_RPC_URL='https://eth-mainnet.g.alchemy.com/v2/<key>'
export SBO3L_SIGNER_KEY='<32-byte hex private key>'
export SBO3L_ALLOW_MAINNET_TX=1                # double-gate

sbo3l agent reputation-publish \
  --fqdn research-agent.sbo3lagent.eth \
  --events events.json \
  --network mainnet \
  --broadcast
```

Mainnet path **additionally requires** `SBO3L_ALLOW_MAINNET_TX=1`
plus an explicit `--network mainnet`. Without the env var, the
command refuses with exit 2 and a "set
`SBO3L_ALLOW_MAINNET_TX=1` to acknowledge before re-running"
message — same double-gate the rest of SBO3L's chain ops use.

Cost ceiling: ~$3-5 per tx at 50 gwei mainnet gas. Per-agent.

## Env vars

| Var                       | Required for          | Purpose                                                  |
|---------------------------|-----------------------|----------------------------------------------------------|
| `SBO3L_RPC_URL`           | `--broadcast` only    | JSON-RPC endpoint (override via `--rpc-url`)             |
| `SBO3L_SIGNER_KEY`        | `--broadcast` only    | 32-byte hex private key (override env via `--private-key-env-var`) |
| `SBO3L_ALLOW_MAINNET_TX`  | `--broadcast --network mainnet` | Must be set to `1`                              |

## Without `eth_broadcast` feature

`--broadcast` still parses, but the dispatch falls through to a
clear "rebuild with `--features eth_broadcast`" stub returning
exit code 3 — same contract as the T-3-1 broadcast stub. The
dry-run output (drop `--broadcast`) is the complete envelope;
operators can pipe its calldata to `cast send` if they prefer not
to rebuild.

## Wire format (what gets signed)

```text
1. setText(node, "sbo3l:reputation_score", "<score>")
   selector: 0x10f13a8c
   args: namehash(fqdn), key, value
   recipient: --resolver (default: network's PublicResolver)
```

The score is computed from the events via
`sbo3l_policy::reputation::compute_reputation_v2` — same v2
4-criteria weighted scoring used by the cross-agent attestation
refusal threshold.

## Verification post-broadcast

After the tx confirms, anyone can verify the record without
SBO3L-specific tooling:

```bash
# Via SBO3L
sbo3l agent verify-ens research-agent.sbo3lagent.eth --network sepolia

# Via cast (no SBO3L dependency)
RESOLVER=$(./target/debug/sbo3l agent reputation-publish \
            --fqdn research-agent.sbo3lagent.eth --events events.json \
            --network sepolia | jq -r .resolver)
NODE=$(cast namehash research-agent.sbo3lagent.eth)
cast call "$RESOLVER" "text(bytes32,string)(string)" "$NODE" "sbo3l:reputation_score" \
  --rpc-url https://ethereum-sepolia-rpc.publicnode.com
```

## Coordination with T-3-1 broadcast

Same alloy stack (`alloy = "0.6"`, gated on the `eth_broadcast`
Cargo feature). Same env-var conventions
(`SBO3L_RPC_URL`, `SBO3L_SIGNER_KEY`). Same Etherscan-link
emission shape. The broadcast harness pattern is shared between
[`agent_broadcast.rs`](../../crates/sbo3l-cli/src/agent_broadcast.rs)
(T-3-1, two txs: setSubnodeRecord + multicall) and
[`agent_reputation_broadcast.rs`](../../crates/sbo3l-cli/src/agent_reputation_broadcast.rs)
(T-4-7, single tx: setText). The two modules deliberately share
*pattern* not *code* for now — a unified broadcast helper is a
follow-up once we have a third caller (likely the AnchorRegistry
publish from P6 / round 9).

## EthLocalFileSigner integration follow-up

This PR uses `PrivateKeySigner::from_bytes` directly, mirroring
T-3-1 broadcast's pattern. The `EthLocalFileSigner` from F-5
(#243) reads the same 32-byte secret format from a *file* rather
than an env var; the two paths converge once T-3-1 broadcast +
T-4-7 broadcast share a unified signer-loading helper. Operators
who prefer file-based keys today can pipe `cat key.hex |
SBO3L_SIGNER_KEY=$(cat /dev/stdin) sbo3l ...` — the env-var path
preserves the existing security model exactly.
