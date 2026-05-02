# Mainnet OffchainResolver — turnkey deploy runbook

> **Status:** **deferred** at hackathon close (2026-05-02).
> Sepolia OffchainResolver at
> `0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3` is live and serves
> the demo. This runbook makes mainnet a paste-runnable
> follow-on if a judge asks, or post-hackathon when monitoring
> is in place.
>
> **Cost ceiling:** ~$5-10 mainnet gas at 50 gwei (deploy +
> `setResolver`).
> **Reversibility:** `setResolver` can be re-pointed to any
> resolver, including the previous PublicResolver. The deployed
> contract can't be unmade, but it costs nothing to leave
> orphaned.
> **Pre-flight gate:** `SBO3L_ALLOW_MAINNET_TX=1` must be set
> explicitly. The deploy script refuses without it.

## Pre-flight checklist

Run each of these *before* the deploy command. Each is a
single line; none costs gas.

### 1. Confirm the gateway signer address matches Vercel env

```bash
# Vercel dashboard → ccip-gateway → env vars → GATEWAY_SIGNER_ADDRESS
# Confirm this matches the address corresponding to GATEWAY_PRIVATE_KEY:
cast wallet address --private-key "$GATEWAY_PRIVATE_KEY"
# → 0x… should match Vercel's GATEWAY_SIGNER_ADDRESS env var
```

If these mismatch, the resolver will accept calls but the
gateway's signatures will be rejected → resolution failure
post-deploy. Fix before continuing.

### 2. Confirm Daniel's deployer wallet has ≥ 0.02 ETH

```bash
cast balance 0xdc7EFA6b4Bd77d1a406DE4727F0DF567e597D231 \
  --rpc-url "$MAINNET_RPC_URL"
# → ≥ 20000000000000000 (2e16 wei = 0.02 ETH)
```

Deploy is ~5e15 wei + each `setResolver` is ~5e14 wei. 0.02 ETH
covers deploy + 7 record set + buffer.

### 3. Confirm the gateway responds at the configured URL

```bash
curl -fsS https://sbo3l-ccip.vercel.app/api/0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3/0x.json
# → 4xx with structured error (not 5xx, not connection refused)
```

The 4xx is *expected* for a malformed `data` arg — what we're
checking is that the gateway is reachable.

### 4. Confirm the existing 7 records on `sbo3lagent.eth` are reproducible

```bash
SBO3L_ENS_RPC_URL="$MAINNET_RPC_URL" \
  sbo3l agent verify-ens sbo3lagent.eth --network mainnet
# → verdict: PASS (all 7 records resolve via current PublicResolver)
```

This is the rollback baseline — what you'd revert to if the
deploy goes wrong.

## Deploy

```bash
export GATEWAY_SIGNER_ADDRESS=0x...               # match Vercel env
export MAINNET_RPC_URL=https://eth-mainnet...     # Daniel's Alchemy key
export SBO3L_DEPLOYER_PRIVATE_KEY=0x...           # dev key, ≥ 0.02 ETH
export NETWORK=mainnet
export SBO3L_ALLOW_MAINNET_TX=1                   # explicit gate

./scripts/deploy-offchain-resolver.sh
# → DEPLOYED:  0x... (note this address)
# → ETHERSCAN: https://etherscan.io/address/0x...
```

The script:
1. Validates the env vars.
2. Refuses without `SBO3L_ALLOW_MAINNET_TX=1`.
3. Runs `forge build` + `forge test` (unit tests against mock signer).
4. Runs `forge create OffchainResolver(gatewaySigner, urls)`.
5. Prints the deployed address + Etherscan link + next-step
   `cast send` snippet.

## Post-deploy: point sbo3lagent.eth at the new resolver

```bash
DEPLOYED=0x...                                    # from previous step
ENS_REGISTRY=0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e
NODE=$(cast namehash sbo3lagent.eth)

cast send "$ENS_REGISTRY" "setResolver(bytes32,address)" \
  "$NODE" "$DEPLOYED" \
  --rpc-url "$MAINNET_RPC_URL" \
  --private-key "$SBO3L_DEPLOYER_PRIVATE_KEY"
```

This is the **point-of-no-return for resolution path** — every
client that doesn't follow CCIP-Read will now fail to resolve
text records on `sbo3lagent.eth` until they upgrade. ENSIP-10
compliance check: viem ≥ 2.0, ethers ≥ 6.0, and ENS App all
follow CCIP-Read.

## Re-issue the 7 canonical records

The OffchainResolver serves records from
`apps/ccip-gateway/data/records.json` — no on-chain
`setText` calls needed. The 7 keys served are:

```
sbo3l:agent_id
sbo3l:endpoint
sbo3l:policy_hash
sbo3l:audit_root
sbo3l:proof_uri
sbo3l:pubkey_ed25519
sbo3l:capabilities
```

After deploy + setResolver, these are immediately served by the
gateway with no further on-chain action. **This is the
runbook's 1-tx improvement over PublicResolver** — 1 tx for
unlimited record updates vs N txes for N updates.

## Post-flight verification

```bash
# Per record:
for KEY in sbo3l:agent_id sbo3l:endpoint sbo3l:policy_hash \
           sbo3l:audit_root sbo3l:proof_uri \
           sbo3l:pubkey_ed25519 sbo3l:capabilities; do
  printf '%s = ' "$KEY"
  # viem.getEnsText follows the CCIP-Read flow end-to-end:
  node -e "
    import('viem/ens').then(async ({getEnsText}) => {
      const {createPublicClient, http} = await import('viem');
      const {mainnet} = await import('viem/chains');
      const client = createPublicClient({chain: mainnet, transport: http(process.env.MAINNET_RPC_URL)});
      const value = await getEnsText(client, {name: 'sbo3lagent.eth', key: '$KEY'});
      console.log(value);
    });
  "
done
```

All 7 should return non-empty strings. If any returns `null`,
the gateway URL or signer address don't match — see rollback.

## Rollback

If post-flight shows any of the 7 records returning `null`:

```bash
PRIOR_RESOLVER=0xF29100983E058B709F3D539b0c765937B804AC15  # mainnet PublicResolver-of-record for sbo3lagent.eth

cast send 0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e \
  "setResolver(bytes32,address)" \
  "$NODE" "$PRIOR_RESOLVER" \
  --rpc-url "$MAINNET_RPC_URL" \
  --private-key "$SBO3L_DEPLOYER_PRIVATE_KEY"
```

This restores the resolution path to the prior PublicResolver
(which still has the original 5 records — the `pubkey_ed25519`
+ `capabilities` Phase 2 records were only ever issued to the
OffchainResolver path, and would need to be re-issued via
PublicResolver if the rollback path becomes durable).

## Pin the deployed address

After mainnet deploy, add the constant to
[`crates/sbo3l-identity/src/contracts.rs`](../crates/sbo3l-identity/src/contracts.rs)
in the "SBO3L deployments" section:

```rust
pub const OFFCHAIN_RESOLVER_MAINNET: ContractPin = ContractPin {
    address: "0x...",                              // from deploy
    network: Network::Mainnet,
    label: "SBO3L OffchainResolver (Mainnet)",
    canonical_source:
        "https://etherscan.io/address/0x...",
};
```

Then add it to `all_pins()`. The `every_pin_is_canonical_form`
+ `no_two_addresses_are_unintentionally_equal` tests will catch
any typo at compile time.

Update [`docs/proof/etherscan-link-pack.md`](proof/etherscan-link-pack.md)
to add the row, and update
[`docs/dev4/closeout-status.md`](dev4/closeout-status.md) to
flip "Deferred" → "Live."

## Why this is deferred at hackathon close

Three conditions to revisit, from R12 P5 + closeout:

1. **Judge specifically asks for mainnet evidence.** Every
   demo flow exercises Sepolia which is read-side identical;
   the test of CCIP-Read isn't network-specific.
2. **Monitoring tooling for the live signer key is in place.**
   Today there's no alerting on signer-key compromise; mainnet
   without alerting is operating without a tripwire.
3. **Rollback rehearsal.** This runbook documents the rollback
   path, but no one has timed it under stress; the safe move is
   to rehearse on a testnet apex first.

Any one of those three flips this from "deferred" to "ship it."
