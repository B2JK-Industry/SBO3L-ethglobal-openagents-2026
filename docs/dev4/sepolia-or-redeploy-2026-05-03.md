# Sepolia OffchainResolver redeploy — 2026-05-03

> **Why this exists:** Heidi's UAT 2026-05-03 caught Bug #2 — the
> Sepolia OffchainResolver at
> [`0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3`](https://sepolia.etherscan.io/address/0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3)
> stored a malformed URL template
> (`"https://sbo3l-ccip.vercel.app/api/{sender/{data}.json}"`)
> instead of the canonical ENSIP-25 form
> (`"https://sbo3l-ccip.vercel.app/api/{sender}/{data}.json"`).
> CCIP-Read clients that follow the spec strictly fail before the
> gateway is even reached.

## Root cause

`forge create --constructor-args "$ADDR" "[$URL]"` rebalances the
`{...}` patterns inside the URL string when tokenizing the array
literal. The closing `}` after `sender` migrated to the end. This
happens regardless of shell quoting — even with
`"[\"$URL\"]"` the same mangling occurs.

## Fix posture

Replaced direct `forge create --constructor-args` invocation with a
forge SCRIPT wrapper at
[`script/DeployOffchainResolver.s.sol`](../../crates/sbo3l-identity/contracts/script/DeployOffchainResolver.s.sol).
The URL template is encoded as a Solidity string literal at file
level (`CANONICAL_URL_TEMPLATE`) so CLI parsing never touches it.

Probe tests at
[`test/DeployOffchainResolver.t.sol`](../../crates/sbo3l-identity/contracts/test/DeployOffchainResolver.t.sol)
pin the constant byte-for-byte, run end-to-end through the
constructor, and confirm both `{sender}` and `{data}` placeholders
survive.

## New live deploy

| Item | Value |
|---|---|
| Address | [`0x87e99508c222c6e419734cacbb6781b8d282b1f6`](https://sepolia.etherscan.io/address/0x87e99508c222c6e419734cacbb6781b8d282b1f6) |
| Network | Sepolia (chain id 11155111) |
| Deployer | `0x50BA7BF5FDe124DB51777A2bF0eED733756B7e9c` (driver wallet) |
| Gateway signer | `0x595099B4e8D642616e298235Dd1248f8008BCe65` (matches Vercel `GATEWAY_PRIVATE_KEY`) |
| Deploy tx | (forge script, see broadcast manifest) |
| Bytecode length | 4747 hex chars |

Verified on chain via PublicNode + Alchemy RPC:

```bash
RPC=https://eth-sepolia.g.alchemy.com/v2/<key>
ADDR=0x87e99508c222c6e419734cacbb6781b8d282b1f6

cast call "$ADDR" "urls(uint256)(string)" 0 --rpc-url "$RPC"
# → "https://sbo3l-ccip.vercel.app/api/{sender}/{data}.json"  ✓ canonical

cast call "$ADDR" "gatewaySigner()(address)" --rpc-url "$RPC"
# → 0x595099B4e8D642616e298235Dd1248f8008BCe65

cast call "$ADDR" "supportsInterface(bytes4)(bool)" 0x9061b923 --rpc-url "$RPC"
# → true   (ENSIP-10 IExtendedResolver)
```

## What this PR does NOT do

- **No `contracts.rs` change.** Pinning the new address into
  `OFFCHAIN_RESOLVER_SEPOLIA` happens in Task C, after Task B
  verifies a working subname E2E. The old address stays pinned in
  this PR so judge-facing docs that reference
  [`0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3`](https://sepolia.etherscan.io/address/0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3)
  don't break mid-cascade.
- **No memory note rewrite.** Same reason — Task C consolidates.
- **No subname registration.** Driver wallet doesn't own
  `sbo3lagent.eth` on Sepolia; that's Task B.

## Decommissioning the old deploy

The old contract at `0x7c6913…aCA8c3` remains on chain
(immutable). Once Task C lands and `OFFCHAIN_RESOLVER_SEPOLIA`
points at the new address, the old contract is orphaned but
harmless — no records on `sbo3lagent.eth` Sepolia ever pointed at
it (driver wallet doesn't own the parent on Sepolia).
