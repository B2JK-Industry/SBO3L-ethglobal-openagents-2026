# Sepolia subname wired to new OR — Heidi UAT bug #2 Task B

> **Heidi UAT bug #2 closeout — Task B.** Demonstrates the
> redeployed Sepolia OffchainResolver
> ([`0x87e99508…b1f6`](https://sepolia.etherscan.io/address/0x87e99508C222c6E419734CACbb6781b8d282b1F6))
> resolves a real subname end-to-end via CCIP-Read, with the
> canonical URL template (`{sender}/{data}.json`) preserved in the
> `OffchainLookup` revert payload.

## What this PR does

1. **Registers `sbo3lagent.eth` on Sepolia** via the V3
   ETHRegistrarController
   ([`0xfb3cE5D0…F968`](https://sepolia.etherscan.io/address/0xfb3cE5D01e0f33f41DbB39035dB9745962F1f968)).
   Driver wallet `0x50BA…7e9c` is now the owner. Registration was
   commit-reveal: 60s minimum age between `commit` and `register`.
2. **Issues `research-agent.sbo3lagent.eth` subname** via
   `setSubnodeRecord` on the ENS Registry, with the new OR as the
   resolver. The owner is the driver wallet; resolver is the
   redeployed OR; ttl = 0.
3. **Adds `crates/sbo3l-identity/tests/sepolia_or_live.rs`** — five
   ignore-gated integration tests that probe the live state.
4. **Adds `scripts/register-sepolia-apex.sh`** — paste-runnable
   wrapper around the commit-reveal + setSubnodeRecord flow,
   idempotent (skips commit if a fresh commitment is already on
   chain).

## On-chain receipts (Sepolia, 2026-05-03)

| Step | Tx | Outcome |
|---|---|---|
| `commit(0xac0dd6…322f)` | (cast send) | commitment recorded, status 1 |
| `register(struct{...})` value=3.4375e15 wei | [`0x655f2b78…1238783`](https://sepolia.etherscan.io/tx/0x655f2b7860d7c435c28ab3904f4a151cf4f485cc90a5f420d307a084b1238783) | apex registered, owner = `0x50BA…7e9c` |
| `setSubnodeRecord(apex, research-agent, owner, OR, 0)` | [`0x71c7fd7b…95db1`](https://sepolia.etherscan.io/tx/0x71c7fd7b2766783e76291060203f542c9df7f4b68d2463315281456bfcb95db1) | subname created, resolver = `0x87e9…b1f6` |

Read-side verified post-registration:

```
namehash(sbo3lagent.eth)            = 0x2e3bac2fc8b574ba1db508588f06102b98554282722141f568960bb66ec12713
ENS.owner(...)                      = 0x50BA7BF5FDe124DB51777A2bF0eED733756B7e9c    ✓ driver wallet

namehash(research-agent.sbo3lagent.eth) = 0x7131b849ffa657c77803cb882a11ea7edaa6e5c2dc2f33f9a878cb1bf39435dd
ENS.owner(...)                          = 0x50BA7BF5FDe124DB51777A2bF0eED733756B7e9c    ✓ driver wallet
ENS.resolver(...)                       = 0x87e99508C222c6E419734CACbb6781b8d282b1F6    ✓ new OR (Task A)
```

## CCIP-Read end-to-end proof

Calling `OR.resolve(dnsEncode(name), text(node, "sbo3l:agent_id"))`
on Sepolia reverts with `OffchainLookup` — the ENSIP-25/EIP-3668
standard signal. Decoding the revert payload:

```
selector  = 0x556f1830                                  ✓ OffchainLookup
sender    = 0x87e99508C222c6E419734CACbb6781b8d282b1F6   ✓ the new OR
urls[0]   = "https://sbo3l-ccip.vercel.app/api/{sender}/{data}.json"  ✓ canonical
callData  = 0x59d1d43c... (text(node, "sbo3l:agent_id"))
extraData = (same as callData, for callback verification)
```

**Heidi UAT bug #2 wire-level fix verified.** The pre-fix OR's
revert payload contained `"...{sender/{data}.json}"`; this OR's
revert payload contains the canonical form. ENSIP-25 clients
(viem ≥ 2, ethers ≥ 6, ENS App) will substitute placeholders
correctly and reach the gateway.

## Tests

```
SBO3L_SEPOLIA_RPC_URL=<sepolia-rpc> \
  cargo test -p sbo3l-identity --test sepolia_or_live -- --ignored
```

Five tests (read-only, no gas):
- `new_or_bytecode_is_live`
- `new_or_url_template_is_canonical` (the bug #2 regression guard)
- `new_or_gateway_signer_matches_vercel`
- `new_or_supports_ensip10_extended_resolver`
- `research_agent_subname_resolver_is_new_or`

All five must pass for Task B's deliverable to be considered live.

## What this PR does NOT do

- ❌ Pin the new OR address in `crates/sbo3l-identity/src/contracts.rs`.
  That's **Task C**. `OFFCHAIN_RESOLVER_SEPOLIA` still points at
  the OLD address `0x7c6913…aCA8c3` until Task C's PR lands.
- ❌ Update `docs/proof/etherscan-link-pack.md` or memory note. Same
  reason — Task C consolidates judge-facing surface.
- ❌ Decommission the old OR contract. It remains on chain; orphaned
  but harmless.

## Cost summary (driver wallet `0x50BA…7e9c`)

| Item | Wei | ETH | USD-ish |
|---|---|---|---|
| commit() gas | ~22,630 × 0.001 gwei | <0.0001 | <$0.01 |
| register() gas + rent | ~3,437,500,000,003,839 | 0.0034 | ~$11 |
| setSubnodeRecord() gas | ~50,000 × 0.001 gwei | <0.0001 | <$0.01 |
| **Total** | | **~0.0034 ETH** | **~$11** |

Driver wallet remaining: ≈ 0.0102 SEP-ETH. Sufficient for any
Task C touch-ups (pure off-chain pin update — no further txes).

## Next: Task C

Pin the new OR (`0x87e99508…b1f6`) in `contracts.rs` + update
`docs/proof/etherscan-link-pack.md` + update memory note
`t41_offchain_resolver_live_2026-05-02.md` with the URL template
fix + working subname proof.
