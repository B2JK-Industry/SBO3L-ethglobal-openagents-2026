# Etherscan link pack — every on-chain claim, one click each

> **Audience:** judges, auditors, partner teams running due
> diligence on the ENS Track submission.
> **Outcome:** scroll once. Every contract address SBO3L either
> deploys, owns, or read-side-resolves against has a clickable
> Etherscan link, the corresponding source-of-truth pin, and the
> Rust constant that exposes it to library consumers.
>
> Single source of truth in the codebase:
> [`crates/sbo3l-identity/src/contracts.rs`](../../crates/sbo3l-identity/src/contracts.rs)
> (the `ContractPin` table). This document is the human-readable
> mirror, regenerated from the same data.

## Mainnet

### Apex name

| Item | Value | Etherscan / ENS App |
|---|---|---|
| ENS name | `sbo3lagent.eth` | [app.ens.domains/sbo3lagent.eth](https://app.ens.domains/sbo3lagent.eth) |
| Namehash | `0x2e3bac2fc8b574ba1db508588f06102b98554282722141f568960bb66ec12713` | — |
| Owner | `0xdc7EFA6b4Bd77d1a406DE4727F0DF567e597D231` | [etherscan.io/address/0xdc7E…D231](https://etherscan.io/address/0xdc7EFA6b4Bd77d1a406DE4727F0DF567e597D231) |
| Resolver | `0xF29100983E058B709F3D539b0c765937B804AC15` | [etherscan.io/address/0xF291…AC15](https://etherscan.io/address/0xF29100983E058B709F3D539b0c765937B804AC15) |

### ENS infrastructure (read by SBO3L; not deployed by SBO3L)

| Contract | Address | Etherscan |
|---|---|---|
| ENS Registry | `0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e` | [etherscan.io/address/0x0000…2e1e](https://etherscan.io/address/0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e) |
| Public Resolver (v3) | `0x231b0Ee14048e9dCcD1d247744d114a4EB5E8E63` | [etherscan.io/address/0x231b…8E63](https://etherscan.io/address/0x231b0Ee14048e9dCcD1d247744d114a4EB5E8E63) |
| Universal Resolver | `0xce01f8eee7E479C928F8919abD53E553a36CeF67` | [etherscan.io/address/0xce01…CF67](https://etherscan.io/address/0xce01f8eee7E479C928F8919abD53E553a36CeF67) |

Pinned in code at:
- `sbo3l_identity::contracts::ENS_REGISTRY`
- `sbo3l_identity::contracts::PUBLIC_RESOLVER_MAINNET`
- `sbo3l_identity::contracts::UNIVERSAL_RESOLVER_MAINNET`

### `sbo3l:*` records on `sbo3lagent.eth` (live, verified 2026-05-01)

```bash
RESOLVER=0xF29100983E058B709F3D539b0c765937B804AC15
NODE=$(cast namehash sbo3lagent.eth)
for KEY in sbo3l:agent_id sbo3l:endpoint sbo3l:policy_hash sbo3l:audit_root sbo3l:proof_uri; do
  printf '%s = ' "$KEY"
  cast call "$RESOLVER" "text(bytes32,string)(string)" "$NODE" "$KEY" \
    --rpc-url https://ethereum-rpc.publicnode.com
done
```

Phase 2 adds `sbo3l:pubkey_ed25519` and `sbo3l:capabilities` for
seven canonical records total. The verification command remains the
same; the loop just gets two more keys.

## Sepolia

### Deployed by SBO3L

| Contract | Address | Etherscan |
|---|---|---|
| **OffchainResolver** (T-4-1, redeploy 2026-05-03 — Heidi UAT bug #2 fix) | `0x87e99508C222c6E419734CACbb6781b8d282b1F6` | [sepolia.etherscan.io/address/0x87e9…b1f6](https://sepolia.etherscan.io/address/0x87e99508C222c6E419734CACbb6781b8d282b1F6) |
| OffchainResolver (T-4-1, ORIGINAL — superseded; URL template malformed, kept for history) | `0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3` | [sepolia.etherscan.io/address/0x7c69…8c3](https://sepolia.etherscan.io/address/0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3) |
| `sbo3lagent.eth` Sepolia apex (registered 2026-05-03) | owner = `0x50BA…7e9c` (driver wallet) | [register tx 0x655f2b78…1238783](https://sepolia.etherscan.io/tx/0x655f2b7860d7c435c28ab3904f4a151cf4f485cc90a5f420d307a084b1238783) |
| `research-agent.sbo3lagent.eth` Sepolia subname | resolver = new OR | [setSubnodeRecord tx 0x71c7fd7b…95db1](https://sepolia.etherscan.io/tx/0x71c7fd7b2766783e76291060203f542c9df7f4b68d2463315281456bfcb95db1) |
| **AnchorRegistry** (R9 P6) | `0x4C302ba8349129bd5963A22e3c7a38a246E8f4Ac` | [sepolia.etherscan.io/address/0x4C30…f4Ac](https://sepolia.etherscan.io/address/0x4C302ba8349129bd5963A22e3c7a38a246E8f4Ac) |
| **SubnameAuction** (R13 P3) | `0x5dE75E64739A95701367F3Ad592e0b674b22114B` | [sepolia.etherscan.io/address/0x5dE7…114B](https://sepolia.etherscan.io/address/0x5dE75E64739A95701367F3Ad592e0b674b22114B) |
| **ReputationBond** (R13 P7) | `0x75072217B43960414047c362198A428f0E9793dA` | [sepolia.etherscan.io/address/0x7507…93dA](https://sepolia.etherscan.io/address/0x75072217B43960414047c362198A428f0E9793dA) |
| **ReputationRegistry** (R11 P1) | `0x6aA95d8126B6221607245c068483fa5008F36dc2` | [sepolia.etherscan.io/address/0x6aA9…6dc2](https://sepolia.etherscan.io/address/0x6aA95d8126B6221607245c068483fa5008F36dc2) |

Pinned in code at:
- `sbo3l_identity::contracts::OFFCHAIN_RESOLVER_SEPOLIA`
- `sbo3l_identity::contracts::ANCHOR_REGISTRY_SEPOLIA`
- `sbo3l_identity::contracts::SUBNAME_AUCTION_SEPOLIA`
- `sbo3l_identity::contracts::REPUTATION_BOND_SEPOLIA`
- `sbo3l_identity::contracts::REPUTATION_REGISTRY_SEPOLIA`

Fuzz suites (10K runs each, foundry invariant tests): see the
corresponding `*.invariant.t.sol` files under
`crates/sbo3l-identity/contracts/test/`.
Off-chain gateway: `https://sbo3l-ccip.vercel.app/api/{sender}/{data}.json`.

Live verification evidence (`cast call` reads against PublicNode RPC,
2026-05-02): [`docs/proof/contracts-live-test.md`](contracts-live-test.md).

### ENS infrastructure (Sepolia counterparts)

| Contract | Address | Etherscan |
|---|---|---|
| ENS Registry | `0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e` | [sepolia.etherscan.io/address/0x0000…2e1e](https://sepolia.etherscan.io/address/0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e) |
| Public Resolver | `0x8FADE66B79cC9f707aB26799354482EB93a5B7dD` | [sepolia.etherscan.io/address/0x8FAD…B7dD](https://sepolia.etherscan.io/address/0x8FADE66B79cC9f707aB26799354482EB93a5B7dD) |
| Universal Resolver | `0xc8Af999e38273D658BE1b921b88A9Ddf005769cC` | [sepolia.etherscan.io/address/0xc8Af…69cC](https://sepolia.etherscan.io/address/0xc8Af999e38273D658BE1b921b88A9Ddf005769cC) |

Pinned in code at:
- `sbo3l_identity::contracts::PUBLIC_RESOLVER_SEPOLIA`
- `sbo3l_identity::contracts::UNIVERSAL_RESOLVER_SEPOLIA`

### Pending (gated on Daniel)

| Contract | Status | Pin | Notes |
|---|---|---|---|
| ERC-8004 IdentityRegistry | **Not yet deployed.** Placeholder at `0x4242…4242`. | `sbo3l_identity::contracts::ERC8004_SEPOLIA_PLACEHOLDER` | Once Daniel deploys, this row flips to the real address; PR #132 lifts from DRAFT and the live AC lights up. |
| OffchainResolver mainnet | **Not yet deployed.** Same script, `NETWORK=mainnet SBO3L_ALLOW_MAINNET_TX=1`. | (constant added on deploy) | Cost ceiling ~$10 mainnet gas. Migration plan for existing 5 records on `sbo3lagent.eth` documented in [`docs/cli/ens-fleet-sepolia.md`](../cli/ens-fleet-sepolia.md). |
| Fleet broadcast (Sepolia) | **Pending Daniel's apex path choice.** | (per-agent tx hashes added on broadcast) | See [`docs/cli/ens-fleet-sepolia.md`](../cli/ens-fleet-sepolia.md) for Path A vs B. |

## Off-chain components (no on-chain address, but contract-of-trust)

| Component | Location | Source |
|---|---|---|
| CCIP-Read gateway | `https://sbo3l-ccip.vercel.app/api/{sender}/{data}.json` | [`apps/ccip-gateway/`](../../apps/ccip-gateway/) |
| GitHub Pages | `https://b2jk-industry.github.io/SBO3L-ethglobal-openagents-2026/` | [`.github/workflows/pages.yml`](../../.github/workflows/pages.yml) |
| Trust DNS visualisation | `apps/trust-dns-viz/bench.html?source=mainnet-fleet` | [`apps/trust-dns-viz/`](../../apps/trust-dns-viz/) |

## Verification one-liners

### "Is the OffchainResolver deployed on Sepolia?"

```bash
cast code 0x87e99508C222c6E419734CACbb6781b8d282b1F6 \
  --rpc-url https://ethereum-sepolia-rpc.publicnode.com | head -c 200
# → 0x6080604052... (non-empty bytecode, deploy confirmed)

# Bonus — check the URL template is canonical (Heidi UAT bug #2 guard):
cast call 0x87e99508C222c6E419734CACbb6781b8d282b1F6 "urls(uint256)(string)" 0 \
  --rpc-url https://ethereum-sepolia-rpc.publicnode.com
# → "https://sbo3l-ccip.vercel.app/api/{sender}/{data}.json"
```

### "Does `sbo3lagent.eth` resolve the canonical records?"

```bash
SBO3L_ENS_RPC_URL=https://ethereum-rpc.publicnode.com \
  sbo3l agent verify-ens sbo3lagent.eth --network mainnet
# → verdict: PASS
```

### "Can a fresh viem client follow the CCIP-Read flow end-to-end?"

```bash
cd examples/t-4-1-viem-e2e && pnpm install && pnpm start
# → three-step flow with gateway response decoded to UTF-8
```

### "Are the contract pins consistent across the codebase?"

```bash
cargo test -p sbo3l-identity --lib contracts
# → 11 tests including cross-checks against ens_live + universal constants
```

## Provenance

This document and `crates/sbo3l-identity/src/contracts.rs` are
synced manually. The Rust module is the canonical source; this
document is regenerated from the `ContractPin` table when an
address changes. A drift between the two surfaces is caught by
manual inspection during PR review — the address constants are
small enough that a one-line change is hard to miss.

Future automation (post-hackathon): a `cargo xtask gen-link-pack`
that emits this file from `contracts.rs::all_pins()`.

## When to update this file

- A new SBO3L deployment (e.g. mainnet OffchainResolver lands) →
  add the row + the Etherscan link + the corresponding Rust
  constant.
- An ENS upgrade (e.g. Universal Resolver v2 ships) → update the
  Universal Resolver row in both this file and `contracts.rs`,
  with `git blame` capturing the rotation.
- A placeholder gets resolved (e.g. ERC-8004 deploy lands) → flip
  the "Pending" row to a regular row in the appropriate network
  section.
