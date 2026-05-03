# Mainnet OffchainResolver + 60-subname fleet — Daniel runbook

> **Audience:** Daniel.
> **Outcome:** mainnet `sbo3lagent.eth` flips to a deployed
> OffchainResolver, 60 subnames are issued under it (50 numbered
> `agent-001..050.sbo3lagent.eth` + 10 specialist roles), each
> with a `sbo3l:agent_id` text record.
> **Cost ceiling:** ~$210 mainnet gas total at 50 gwei.
> **Time:** <30 min including pre-flight + verification.
> **Reversibility:** `setResolver` step reversible by reverting to
> the prior PublicResolver `0xF291…AC15`. Subname registrations
> are reversible (set resolver to `0x0`) but cost-asymmetric — you
> spent gas to issue, would spend gas again to clear.

---

## STEP 0 — Pre-flight (read-only, no gas)

```bash
# Set the env vars used by every later step.
export MAINNET_RPC_URL=https://eth-mainnet.g.alchemy.com/v2/n9pLYLbfcNRkZXVs7Togt
export ENS_REGISTRY=0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e
export GATEWAY_SIGNER_ADDRESS=0x595099B4e8D642616e298235Dd1248f8008BCe65
export DANIEL_WALLET=0xdc7EFA6b4Bd77d1a406DE4727F0DF567e597D231
export NODE=$(cast namehash sbo3lagent.eth)

# 0.1 — Verify Daniel wallet has > 0.013 ETH on mainnet
cast balance "$DANIEL_WALLET" --rpc-url "$MAINNET_RPC_URL"
# expect: > 13000000000000000 (= 0.013 ETH; ~$45 buffer over $210 cost ceiling)

# 0.2 — Verify gateway signer matches what's baked into the existing Sepolia OR.
# (If they don't match, the new mainnet OR will be deployed with a stale signer
# and the gateway can't sign for it — STOP + investigate.)
cast call 0x87e99508C222c6E419734CACbb6781b8d282b1F6 "gatewaySigner()(address)" \
  --rpc-url https://ethereum-sepolia-rpc.publicnode.com
# expect: 0x595099B4e8D642616e298235Dd1248f8008BCe65

# 0.3 — Snapshot current mainnet sbo3lagent.eth resolver (rollback target).
PRIOR_RESOLVER=$(cast call "$ENS_REGISTRY" "resolver(bytes32)(address)" "$NODE" \
  --rpc-url "$MAINNET_RPC_URL")
echo "PRIOR_RESOLVER=$PRIOR_RESOLVER"
# expect: 0xF29100983E058B709F3D539b0c765937B804AC15
# Save this — it's the target for STEP 5 rollback.

# 0.4 — Confirm the gateway is reachable.
curl -fsS https://sbo3l-ccip.vercel.app/api/0x87e99508C222c6E419734CACbb6781b8d282b1F6/0x.json
# expect: 4xx with structured error (proves gateway is up)
```

If any of 0.1–0.4 fails, STOP and tell me — don't proceed.

---

## STEP 1 — Deploy mainnet OffchainResolver (~$5 gas)

```bash
# Set Daniel's primary wallet PK. Don't paste this anywhere
# else; it leaves the shell when you exit.
export SBO3L_DEPLOYER_PRIVATE_KEY=<paste-Daniel-primary-wallet-PK>

# Mainnet gate — the deploy script refuses without this env var.
export NETWORK=mainnet
export SBO3L_ALLOW_MAINNET_TX=1

# Run the existing deploy script. Same script that produced the
# Sepolia OR at 0x87e99508…b1f6.
cd /Users/danielbabjak/Desktop/MandateETHGlobal/mandate-ethglobal-openagents-2026
./scripts/deploy-offchain-resolver.sh
```

The script:
1. Validates env vars.
2. Refuses without `SBO3L_ALLOW_MAINNET_TX=1`.
3. Runs forge build + tests.
4. Deploys via the forge script wrapper (URL-template canonical
   form pinned in Solidity, no CLI brace-rebalancing).
5. Prints `DEPLOYED: 0x...` + Etherscan link.

**Save the deployed address into your shell:**

```bash
export DEPLOYED_OR=0x...   # from script output
```

---

## STEP 2 — `setResolver` on `sbo3lagent.eth` (~$3 gas)

⚠️ **This is the point-of-no-return for resolution.** Once
`setResolver` lands, every ENS client that doesn't follow
CCIP-Read will fail to resolve text records on `sbo3lagent.eth`
until they upgrade to ENSIP-10. ENSIP-10 compliance: viem ≥ 2.0,
ethers ≥ 6.0, ENS App.

```bash
cast send "$ENS_REGISTRY" \
  "setResolver(bytes32,address)" \
  "$NODE" "$DEPLOYED_OR" \
  --rpc-url "$MAINNET_RPC_URL" \
  --private-key "$SBO3L_DEPLOYER_PRIVATE_KEY"
```

Verify:

```bash
cast call "$ENS_REGISTRY" "resolver(bytes32)(address)" "$NODE" \
  --rpc-url "$MAINNET_RPC_URL"
# expect: $DEPLOYED_OR
```

---

## STEP 3 — Register 60 subnames batch (~$200 gas)

The 60 subnames break into:

- **50 numbered**: `agent-001.sbo3lagent.eth` ... `agent-050.sbo3lagent.eth`
- **10 specialist**: `research`, `trader`, `auditor`, `compliance`,
  `treasury`, `analytics`, `reputation`, `oracle`, `messenger`,
  `executor` — each as `<role>.sbo3lagent.eth`

Each subname needs:
1. `setSubnodeRecord(parent, label, owner, resolver, ttl)` on ENS
   Registry — pins `<sub>.sbo3lagent.eth` to the new OR.
2. `setText(node, "sbo3l:agent_id", "<sub>")` on the OR's gateway —
   handled by the gateway's `records.json` (no on-chain text-set
   needed; the OR is CCIP-Read).

Because the OR is CCIP-Read, **only step 1** is on-chain. The
records themselves are served by `apps/ccip-gateway/data/records.json`.
That file already has `research-agent.sbo3lagent.eth` baked in —
we'll add the 60 entries before broadcasting.

### 3a — Update gateway records.json (off-chain, no gas)

```bash
cd apps/ccip-gateway
node scripts/seed-fleet-records.mjs > data/records.json.new
mv data/records.json.new data/records.json
git diff data/records.json | head -20
```

(I'll ship `seed-fleet-records.mjs` as part of Task A's PR — it
generates the 60 entries deterministically. Until it merges, the
gateway has only the 2 demo records; STEP 4 verify-ens will only
return values for those 2.)

Once happy with the diff, push to Vercel:

```bash
vercel deploy --prod --cwd apps/ccip-gateway
```

### 3b — Multicall the 60 setSubnodeRecord calls (on-chain)

The PublicResolver supports `multicall(bytes[])` but the ENS
Registry doesn't directly. Use the
[ENS Multicall](https://docs.ens.domains/web/quickstart) wrapper
or batch-as-script. SBO3L ships a forge script:

```bash
export PRIVATE_KEY="$SBO3L_DEPLOYER_PRIVATE_KEY"
export PARENT_NODE="$NODE"
export RESOLVER_ADDRESS="$DEPLOYED_OR"

cd crates/sbo3l-identity/contracts
forge script script/RegisterMainnetFleet.s.sol \
  --rpc-url "$MAINNET_RPC_URL" \
  --broadcast --slow
```

(I'll ship `RegisterMainnetFleet.s.sol` as part of Task A's PR.
It loops over the 60 labels + emits 60 `setSubnodeRecord` calls
in one transaction batch via `vm.startBroadcast` + the standard
Foundry batch optimisation. Total cost ≈ 60 × ~$3 = ~$180 at 50
gwei. With 10% buffer = ~$200.)

The script prints each subname namehash so you can spot-check.

---

## STEP 4 — Verify

### 4a — Spot-check 3 records via SBO3L CLI

```bash
SBO3L_ENS_RPC_URL="$MAINNET_RPC_URL" \
  sbo3l agent verify-ens research.sbo3lagent.eth --network mainnet
# expect: verdict: PASS, agent_id = "research"

SBO3L_ENS_RPC_URL="$MAINNET_RPC_URL" \
  sbo3l agent verify-ens agent-001.sbo3lagent.eth --network mainnet
# expect: verdict: PASS, agent_id = "agent-001"

SBO3L_ENS_RPC_URL="$MAINNET_RPC_URL" \
  sbo3l agent verify-ens trader.sbo3lagent.eth --network mainnet
# expect: verdict: PASS, agent_id = "trader"
```

### 4b — Raw cast verification (no SBO3L binary)

```bash
for SUBNAME in research trader auditor agent-001 agent-025 agent-050; do
  SUBNODE=$(cast namehash "$SUBNAME.sbo3lagent.eth")
  printf '%s = ' "$SUBNAME"
  cast call "$DEPLOYED_OR" "text(bytes32,string)(string)" "$SUBNODE" "sbo3l:agent_id" \
    --rpc-url "$MAINNET_RPC_URL"
done
```

(`cast call` will trigger CCIP-Read; alloy/cast handle the gateway
fetch transparently.)

If any return null/empty, see **STEP 5 rollback**.

---

## STEP 5 — Rollback (if STEP 4 fails)

If verify-ens fails or returns empty for canonical records:

```bash
cast send "$ENS_REGISTRY" \
  "setResolver(bytes32,address)" \
  "$NODE" "$PRIOR_RESOLVER" \
  --rpc-url "$MAINNET_RPC_URL" \
  --private-key "$SBO3L_DEPLOYER_PRIVATE_KEY"
```

This restores `sbo3lagent.eth` to the prior PublicResolver
`0xF29100983E058B709F3D539b0c765937B804AC15`. The original 5
records on the apex (`agent_id`, `endpoint`, `policy_hash`,
`audit_root`, `proof_uri`) were never deleted; they remain on
the PublicResolver and become live again.

The 60 subnames you registered remain on chain (resolver pointing
at the new OR). They'll resolve `0x0` until you either:
- Re-run the OR deploy + re-setResolver (preferred — fix and retry).
- Set each subname's resolver back to `0x0` via 60 `setSubnodeRecord`
  calls (~$180 wasted, last-resort).

The new OR contract itself remains on chain (immutable). It's
orphaned but harmless.

---

## STEP 6 — After verify (Daniel-side done; my-side starts)

Once STEP 4 returns expected values for at least 3 of the 6
spot-checks above, paste the deployed address into our chat and
I'll run a 5-minute follow-up PR:

1. Pin `OFFCHAIN_RESOLVER_MAINNET` in
   `crates/sbo3l-identity/src/contracts.rs`.
2. Update `docs/proof/etherscan-link-pack.md` with the mainnet
   row.
3. Update `docs/dev4/closeout-status.md` to flip "Mainnet OR
   deferred" → "Live".
4. Update memory `t41_offchain_resolver_live_2026-05-02.md` with
   the new mainnet address.
5. Update `docs/submission/bounty-ens-most-creative-final.md` —
   add mainnet OR row to submission metadata table.

Total < 5 min, single PR, judge-clickable evidence flips from
"deployed Sepolia + deferred mainnet" to "live on both networks
+ 60-name fleet."

---

## Dependencies this PR ships

This PR adds:

- `apps/ccip-gateway/scripts/seed-fleet-records.mjs` — generates
  the 60 records.json entries deterministically. Each entry
  carries `sbo3l:agent_id = <subname>` minimum. Runs locally,
  outputs to stdout. Idempotent.
- `crates/sbo3l-identity/contracts/script/RegisterMainnetFleet.s.sol`
  — forge script that issues 60 `setSubnodeRecord` calls in one
  broadcast. Reads `PARENT_NODE`, `RESOLVER_ADDRESS`,
  `PRIVATE_KEY` from env. Configurable label list via solidity
  string array (50 numbered + 10 specialist hardcoded).
- `crates/sbo3l-identity/contracts/test/RegisterMainnetFleet.t.sol`
  — foundry test that mocks the Registry + verifies the 60
  setSubnodeRecord calls land with correct (label, owner,
  resolver, ttl) tuples.

These are paste-runnable scripts, not just docs.

---

## Cost summary

| Step | Op | Gas | $ at 50 gwei |
|---|---|---|---|
| 1 | OR deploy | ~1.5M | ~$5 |
| 2 | setResolver | ~80K | ~$3 |
| 3 | 60× setSubnodeRecord (multicall) | ~3M total | ~$180 |
| **Total** | | | **~$190** |

Daniel's wallet has 0.014 ETH ≈ $50 mainnet — **insufficient**.
Need top-up of at least 0.05 ETH (≈ $170) before STEP 3. OR run
STEP 1 + 2 only with current funds (~$8) and defer the 60-name
fleet.

⚠️ **If you run STEP 1+2 only**: `sbo3lagent.eth` apex itself
will work via the new OR (CCIP-Read serves the existing 5 records
on the apex from `records.json`), but the 60 subnames won't exist
and verify-ens against `agent-001.sbo3lagent.eth` returns
"namehash unknown."

The deferred-fleet posture is documented in
`docs/dev4/closeout-status.md` already; no emergency.

---

## Why this matters (judge-grade impact)

- **ENS Most Creative**: from "we built a thing on Sepolia" to
  "60-agent constellation live on mainnet." Closes the
  "production-shaped scale" claim in the bounty narrative.
- **Cross-track**: the live mainnet apex makes the ENSIP-26
  upstream PR ([ensdomains/ensips#71](https://github.com/ensdomains/ensips/pull/71))
  citable as "reference impl shipping at scale" rather than
  "demo deployment."
- **Truthfulness gate**: every "60-agent fleet" claim in the
  submission narrative becomes resolvable from a public RPC + cast.
