# Live contract verification — Sepolia, 2026-05-02

> **Audience:** judges + auditors. This file documents read-side
> verification that every SBO3L Sepolia deployment is on-chain
> and answering its public ABI as expected. Every command below
> is paste-ready against PublicNode's open RPC; no API key
> needed.
>
> **Method:** `cast call` reads against
> `https://ethereum-sepolia-rpc.publicnode.com`. Each contract
> gets:
> 1. **Bytecode length check** (proves deploy landed).
> 2. **`supportsInterface(0x01ffc9a7)`** (proves ERC-165
>    self-introspection wired).
> 3. **One representative public-state read** (proves the
>    intended ABI is on-chain, not just *some* bytecode).
>
> **Write-tx checks** are listed as optional follow-ups gated on
> a funded private key; the read evidence alone is enough to
> verify the on-chain surface.

## Summary table

| Contract | Address | Bytecode | ERC-165 | Sample state |
|---|---|---|---|---|
| AnchorRegistry | `0x4C302ba8349129bd5963A22e3c7a38a246E8f4Ac` | 3308 chars | ✅ true | `anchorCount(0x00…) = 0` |
| SubnameAuction | `0x5dE75E64739A95701367F3Ad592e0b674b22114B` | 8934 chars | ✅ true | `auctionCount = 0`, `MIN_INCREMENT_BPS = 500`, `MIN_DURATION = 3600` |
| ReputationBond | `0x75072217B43960414047c362198A428f0E9793dA` | 5368 chars | ✅ true | `BOND_AMOUNT = 1e16` (0.01 ETH), `LOCK_PERIOD = 604800` (7d), `slasher = 0x50BA…7e9c`, `insuranceBeneficiary = 0xdc7E…D231`, `insurancePool = 0` |
| ReputationRegistry | `0x6aA95d8126B6221607245c068483fa5008F36dc2` | 6024 chars | ✅ true | `tenantSigner(0x00…) = 0x0…0` |
| OffchainResolver (R9 baseline) | `0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3` | non-empty | ✅ true | (verified in earlier rounds; see judge walkthrough) |

All five are pinned in
[`crates/sbo3l-identity/src/contracts.rs`](../../crates/sbo3l-identity/src/contracts.rs)
and surface through `sbo3l_identity::contracts::all_pins()`.

## Per-contract evidence

### AnchorRegistry (`0x4C302ba8349129bd5963A22e3c7a38a246E8f4Ac`)

```bash
RPC=https://ethereum-sepolia-rpc.publicnode.com
ADDR=0x4C302ba8349129bd5963A22e3c7a38a246E8f4Ac

# 1. Bytecode landed
cast code "$ADDR" --rpc-url "$RPC" | wc -c
# → 3308

# 2. ERC-165 self-introspection
cast call "$ADDR" "supportsInterface(bytes4)(bool)" 0x01ffc9a7 --rpc-url "$RPC"
# → true

# 3. Sample state read — anchor count for the zero address (clean state)
cast call "$ADDR" "anchorCount(address)(uint256)" \
  0x0000000000000000000000000000000000000000 --rpc-url "$RPC"
# → 0
```

**Interpretation.** Deploy confirmed; anchor table is empty as
expected for a fresh contract; ERC-165 wiring intact.

### SubnameAuction (`0x5dE75E64739A95701367F3Ad592e0b674b22114B`)

```bash
ADDR=0x5dE75E64739A95701367F3Ad592e0b674b22114B

cast code "$ADDR" --rpc-url "$RPC" | wc -c
# → 8934

cast call "$ADDR" "supportsInterface(bytes4)(bool)" 0x01ffc9a7 --rpc-url "$RPC"
# → true

cast call "$ADDR" "auctionCount()(uint256)" --rpc-url "$RPC"
# → 0
cast call "$ADDR" "MIN_INCREMENT_BPS()(uint16)" --rpc-url "$RPC"
# → 500
cast call "$ADDR" "MIN_DURATION()(uint256)" --rpc-url "$RPC"
# → 3600
```

**Interpretation.** No auctions yet; the constants exposed match
the contract source (5% minimum increment, 1-hour minimum
duration). These constants are themselves a public correctness
check that the deployed bytecode came from the expected source
build, not a different commit.

### ReputationBond (`0x75072217B43960414047c362198A428f0E9793dA`)

```bash
ADDR=0x75072217B43960414047c362198A428f0E9793dA

cast code "$ADDR" --rpc-url "$RPC" | wc -c
# → 5368

cast call "$ADDR" "supportsInterface(bytes4)(bool)" 0x01ffc9a7 --rpc-url "$RPC"
# → true

cast call "$ADDR" "BOND_AMOUNT()(uint256)" --rpc-url "$RPC"
# → 10000000000000000          (1e16 wei = 0.01 ETH)
cast call "$ADDR" "LOCK_PERIOD()(uint256)" --rpc-url "$RPC"
# → 604800                     (7 days)
cast call "$ADDR" "slasher()(address)" --rpc-url "$RPC"
# → 0x50BA7BF5FDe124DB51777A2bF0eED733756B7e9c
cast call "$ADDR" "insuranceBeneficiary()(address)" --rpc-url "$RPC"
# → 0xdc7EFA6b4Bd77d1a406DE4727F0DF567e597D231     (Daniel)
cast call "$ADDR" "insurancePool()(uint256)" --rpc-url "$RPC"
# → 0
```

**Interpretation.** Deployed against the expected constructor
parameters: `slasher` = governance multisig placeholder,
`insuranceBeneficiary` = Daniel's deployer EOA. Bond size and
lock period match contract source. Insurance pool starts at
zero — fills as slashes execute.

### ReputationRegistry (`0x6aA95d8126B6221607245c068483fa5008F36dc2`)

```bash
ADDR=0x6aA95d8126B6221607245c068483fa5008F36dc2

cast code "$ADDR" --rpc-url "$RPC" | wc -c
# → 6024

cast call "$ADDR" "supportsInterface(bytes4)(bool)" 0x01ffc9a7 --rpc-url "$RPC"
# → true

cast call "$ADDR" "tenantSigner(bytes32)(address)" \
  0x0000000000000000000000000000000000000000000000000000000000000000 --rpc-url "$RPC"
# → 0x0000000000000000000000000000000000000000
```

**Interpretation.** No tenant has claimed yet — the zero-key
slot maps to the zero address. The first `setTenantSigner` call
will populate this slot; the read is a safe sanity check that
the storage layout matches the ABI.

## Optional: write-tx evidence

The above is sufficient for verification of *deploy + ABI
correctness*. End-to-end behavioural correctness (a full
`createAuction → bid → settle` round-trip, or a full
`postAnchor → verify` round-trip) can be exercised from a funded
EOA against the same Sepolia addresses. The runbooks for those
flows live alongside their CLI subcommands:

- AnchorRegistry: `sbo3l agent post-anchor --broadcast` →
  [`docs/cli/anchor-registry.md`](../cli/anchor-registry.md)
- ReputationRegistry: `sbo3l agent reputation-publish --broadcast`
  → already exercised in R12 broadcast doc.

The closeout posture is to **defer write-tx demos** to live
demo time (not committed to the repo) so judges see them
fresh; the read-side evidence here is the "did the contract
actually deploy" gate.

## RPC choice

`https://ethereum-sepolia-rpc.publicnode.com` is the chosen
verification endpoint — open, no key, stable in our prior
testing across multiple sessions. If PublicNode is temporarily
down at demo time, swap in any of:

- `https://eth-sepolia.g.alchemy.com/v2/<key>` (Daniel's pinned
  Alchemy key — see memory `alchemy_rpc_endpoints.md`)
- `https://rpc.sepolia.org` (slow but works)
- `https://eth-sepolia.public.blastapi.io`

The same `cast` commands work against any of them — the address
+ ABI is the source of truth, not the RPC provider.
