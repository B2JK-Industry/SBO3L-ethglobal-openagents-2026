# T-4-1 OffchainResolver — Deploy Runbook

**Audience:** Daniel (or any operator with the SBO3L deployer wallet
key + Vercel admin access). Solidity / Foundry experience helpful but
not required.

**Outcome:** in ~3-4 minutes of wallet ops, an OffchainResolver
contract is live on Sepolia at a pinned address, the `sbo3lagent.eth`
parent points at it, and `viem.getEnsText("<agent>.sbo3lagent.eth",
"sbo3l:agent_id")` returns the agent's `sbo3l:*` record set via
CCIP-Read.

This runbook unblocks the ops half of T-4-1: the gateway code is
already merged via #124; the contract + ENS pointer flip are the only
on-chain bits remaining.

## Prereqs (one-time)

1. **Foundry installed.** `curl -L https://foundry.paradigm.xyz | bash && foundryup`.
2. **`apps/ccip-gateway` deployed to Vercel** (see
   `apps/ccip-gateway/DEPLOY.md`). The deploy is the same `GATEWAY_PRIVATE_KEY`
   we'll bake into the resolver here.
3. **Sepolia RPC URL.** Alchemy / Infura / PublicNode all work; the
   memory note `alchemy_rpc_endpoints.md` has Daniel's Alchemy key.
4. **Funded Sepolia deployer wallet** (~0.05 ETH covers deploy +
   `setResolver`). Memory note confirms `0xdc7EFA…D231` has 0.1
   SEP-ETH.

## Step 1 — Read the gateway signer address

The OffchainResolver bakes one address at deploy time: the
secp256k1 address whose private key the gateway uses to sign
responses. To match what's in Vercel:

```bash
# In apps/ccip-gateway, during local dev with .env.local set:
cd apps/ccip-gateway
node -e "
  const { privateKeyToAccount } = require('viem/accounts');
  const dotenv = require('dotenv'); dotenv.config({ path: '.env.local' });
  console.log(privateKeyToAccount(process.env.GATEWAY_PRIVATE_KEY).address);
"
# → prints 0x... — pin this as GATEWAY_SIGNER_ADDRESS for step 3
```

Or, equivalently, log into Vercel → project `sbo3l-ccip` → Settings →
Environment Variables → reveal `GATEWAY_PRIVATE_KEY` for production →
derive the address with the same one-liner against the leaked key.

## Step 2 — Verify the contract compiles + tests pass

From the repo root:

```bash
cd crates/sbo3l-identity/contracts
forge install foundry-rs/forge-std@v1.10.0 --no-commit  # one-time
forge build
forge test
# expect:
#   [PASS] test_resolve_reverts_with_offchain_lookup
#   [PASS] test_callback_with_valid_signature_returns_value
#   [PASS] test_callback_rejects_expired_signature
#   [PASS] test_callback_rejects_unauthorized_signer
#   [PASS] test_callback_rejects_tampered_value
#   [PASS] test_supports_interface
```

`forge-std` lives under `crates/sbo3l-identity/contracts/lib/` and is
git-ignored (per `.gitignore` in that directory). Reinstall on every
fresh clone via `forge install`.

## Step 3 — Deploy to Sepolia

```bash
export GATEWAY_SIGNER_ADDRESS=0x...                    # from step 1
export SEPOLIA_RPC_URL=https://eth-sepolia.g.alchemy.com/v2/...
export SBO3L_DEPLOYER_PRIVATE_KEY=0x...                # 0xdc7EFA…D231 dev key

./scripts/deploy-offchain-resolver.sh

# Output ends with:
#   DEPLOYED:  0xResolverAddressHere
#   ETHERSCAN: https://sepolia.etherscan.io/address/0xResolverAddressHere
```

The script:
1. Validates env vars.
2. Lazily installs `forge-std` if missing.
3. Runs `forge build` + `forge test`.
4. Calls `forge create` with constructor args `(GATEWAY_SIGNER_ADDRESS,
   ["https://sbo3l-ccip.vercel.app/api/{sender}/{data}.json"])`.
5. Prints the address + Etherscan link.

Cost: ~0.005 SEP-ETH at 30 gwei, free testnet gas.

## Step 4 — Point sbo3lagent.eth at the resolver

```bash
ENS_REGISTRY=0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e
PARENT=$(cast namehash sbo3lagent.eth)
DEPLOYED_RESOLVER=0xResolverAddressHere   # from step 3

cast send $ENS_REGISTRY \
  "setResolver(bytes32,address)" \
  $PARENT \
  $DEPLOYED_RESOLVER \
  --rpc-url $SEPOLIA_RPC_URL \
  --private-key $SBO3L_DEPLOYER_PRIVATE_KEY
```

This makes every subname under `sbo3lagent.eth` resolve through the
OffchainResolver (ENS inherits the parent's resolver for subnames
without their own resolver set). For T-4-1 we leave per-subname
resolvers unset so all of them share this OffchainResolver.

## Step 5 — Verify end-to-end with viem

```bash
cd apps/ccip-gateway
node -e "
  import('viem').then(({ createPublicClient, http }) =>
    import('viem/chains').then(async ({ sepolia }) => {
      const c = createPublicClient({
        chain: sepolia,
        transport: http(process.env.SEPOLIA_RPC_URL),
      });
      const value = await c.getEnsText({
        name: 'research-agent.sbo3lagent.eth',
        key:  'sbo3l:agent_id',
      });
      console.log('value:', value);
    })
  );
"
# expect: value: research-agent-01
```

If this prints the expected value, T-4-1's E2E flow is live: viem
issues `text(node, key)` against the resolver, the resolver reverts
with `OffchainLookup`, viem fetches from the gateway, viem submits the
signed response back via `resolveCallback`, the resolver verifies the
signature on-chain and returns the decoded value. All without any
SBO3L-specific client code.

## Step 6 — Pin the deployed address

Update this doc replacing `TBD` with the actual Sepolia deployment:

| Network  | OffchainResolver address                       | Etherscan                                                              |
|----------|------------------------------------------------|------------------------------------------------------------------------|
| Sepolia  | `TBD` (paste from step 3)                       | TBD                                                                    |
| Mainnet  | not deployed                                    | n/a (Phase 3 amplifier; mainnet path needs `SBO3L_ALLOW_MAINNET_TX=1`) |

Also pin in `live_rpc_endpoints_known.md` memory note so future Dev 4
sessions can re-derive without re-reading this doc.

## Rotating the gateway signer

If `GATEWAY_PRIVATE_KEY` ever leaks (or just for hygiene):

1. Generate a new key in Vercel env (preview + production +
   development).
2. Redeploy `apps/ccip-gateway` (auto on next push to main).
3. **Redeploy this OffchainResolver with the new
   `GATEWAY_SIGNER_ADDRESS`** — the address is `immutable`, so a new
   contract is the only path. Cost: ~0.005 SEP-ETH testnet, ~$15
   mainnet.
4. Re-run step 4 to point `sbo3lagent.eth` at the new resolver.
5. The old resolver still works for the time it serves but with a
   stale signer key; clients will see signature-verification failures
   until you complete step 4.

## Troubleshooting

- **`forge create` reverts with `InvalidSignerLength`** — the
  constructor refuses `address(0)`. Re-derive `GATEWAY_SIGNER_ADDRESS`
  from the right key.
- **`viem.getEnsText` returns `null`** — the resolver isn't pointing
  at the OffchainResolver, OR the gateway URL is wrong, OR the
  signer address baked at deploy doesn't match the gateway's current
  signing key. Run `cast call $ENS_REGISTRY "resolver(bytes32)"
  $(cast namehash sbo3lagent.eth) --rpc-url $SEPOLIA_RPC_URL` and
  compare to the deployed resolver address.
- **`viem.getEnsText` errors with `signature expired`** — gateway
  clock skew vs the chain's `block.timestamp`. The TTL in
  `apps/ccip-gateway/src/lib/sign.ts` is 60s by default; bump
  `DEFAULT_TTL_SECONDS` if Vercel function cold-start latency is
  routinely >60s.
- **`viem.getEnsText` errors with `unauthorized signer`** — the
  signer baked into the resolver doesn't match the address derived
  from `GATEWAY_PRIVATE_KEY`. Either rotate the resolver (step 6) or
  rotate the gateway key to match.

## References

- ENSIP-10: `https://docs.ens.domains/ensip/10`
- EIP-3668 (CCIP-Read): `https://eips.ethereum.org/EIPS/eip-3668`
- EIP-191 (intended validator): `https://eips.ethereum.org/EIPS/eip-191#0x19-prefix`
- ENS Labs reference OffchainResolver:
  `https://github.com/ensdomains/offchain-resolver/blob/main/packages/contracts/contracts/OffchainResolver.sol`
- T-4-1 design doc: `docs/design/T-4-1-ccip-read-prep.md`
- Gateway deploy doc: `apps/ccip-gateway/DEPLOY.md`
