# T-4-5 — ENS Universal Resolver integration

**Status:** Shipped (this PR).
**Track:** ENS — Phase 2 production hardening.
**Crate:** `crates/sbo3l-identity/src/universal.rs`.

## Why

`LiveEnsResolver` (T-4-1) does **1 + 5 = 6 RPC calls** to read every
agent's full SBO3L text-record set:

1. `ENSRegistry.resolver(node)` → resolver address.
2. `Resolver.text(node, "sbo3l:agent_id")`.
3. `Resolver.text(node, "sbo3l:endpoint")`.
4. `Resolver.text(node, "sbo3l:policy_hash")`.
5. `Resolver.text(node, "sbo3l:audit_root")`.
6. `Resolver.text(node, "sbo3l:proof_uri")`.

Every fleet-resolution pass (5 agents → 30 calls; 60 agents → 360 calls)
multiplies that cost on whichever public RPC the operator points at.
Free public RPCs throttle aggressively; the 60-agent workflow brushes
limits.

The ENS Universal Resolver collapses this to **one** `eth_call`:

```text
UniversalResolver.resolve(
    dnsEncode("sbo3lagent.eth"),
    multicall([
        text(node, "sbo3l:agent_id"),
        text(node, "sbo3l:endpoint"),
        text(node, "sbo3l:policy_hash"),
        text(node, "sbo3l:audit_root"),
        text(node, "sbo3l:proof_uri"),
    ])
)
→ (bytes result, address resolver)
```

`result` is the ABI-encoded `bytes[]` from the inner multicall;
each entry is an ABI-encoded `(string)` tuple. We decode all three
layers in one Rust pass (`UniversalResolver::resolve_all`).

## Scope: on-chain resolver fast path

This module is a **strict optimisation**. It applies when the
resolver registered for the name implements
`Multicallable.multicall(bytes[])` — exactly the shape of the
canonical ENS PublicResolver, where `sbo3lagent.eth`'s five records
are stored on mainnet today.

For names backed by an ENSIP-10 OffchainResolver (Sepolia subnames
served via the SBO3L CCIP-Read gateway), the universal resolver
propagates the inner `OffchainLookup` revert; the existing
`LiveEnsResolver` + `ccip_read` flow remains the right tool there.
We classify that case explicitly and surface
`UniversalError::OffchainResolverRequiresCcipFlow` so callers can
fall through cleanly.

## Address pinning

| Network | Universal Resolver address                       |
|---------|--------------------------------------------------|
| Mainnet | `0xce01f8eee7E479C928F8919abD53E553a36CeF67`     |
| Sepolia | `0xc8Af999e38273D658BE1b921b88A9Ddf005769cC`     |

Same constants `viem` ships with. Override with
`UniversalResolver::with_address` if a future ENS deployment moves
either contract — the ABI is the stable contract-of-trust here, not
the address.

## Wildcard + DNSSEC

Both are handled inside the universal resolver itself
(ENSIP-10 wildcard + DNSSEC bridging at the contract level). Our
client doesn't need special cases — the same single `eth_call`
resolves a regular name, a wildcard child of an ENSIP-10 parent, OR
a DNS name proxied via ENS.

## Round-trip count

| Path                                         | Calls (pre) | Calls (post) |
|----------------------------------------------|-------------|--------------|
| Single agent, on-chain resolver              | 6           | **1**        |
| 5-agent fleet, on-chain resolver             | 30          | **5**        |
| 60-agent fleet, on-chain resolver            | 360         | **60**       |
| Single agent, OffchainResolver (CCIP-Read)   | 1 + 1       | n/a — falls back to `LiveEnsResolver` |

For the live demo against `sbo3lagent.eth` on mainnet, the universal
resolver path is what reduces the public-RPC pressure to a level
that fits inside a single judging window.

## Tests

`crates/sbo3l-identity/src/universal.rs` ships 13 unit tests:

- DNS-encode happy path + edge cases (empty name, trailing dot,
  64-byte label rejection).
- `UNIVERSAL_RESOLVE_SELECTOR` and `MULTICALL_SELECTOR` recomputed
  against `keccak256(...)[..4]` so neither can silently drift.
- `encode_multicall` / `encode_universal_resolve` round-trip
  (encode then decode — bit-exact recovery).
- Happy-path: hand-built canned response containing five known
  `(string)` tuples → `UniversalResolver::resolve_all` returns the
  matching `EnsRecords`.
- Empty record → `MissingRecord(field)` (not silently empty).
- OffchainLookup classification heuristic (selector + textual forms).
- Address-override smoke test.

All tests pass under `cargo test -p sbo3l-identity --lib`.

## Public surface

```rust
use sbo3l_identity::{
    EnsNetwork, ReqwestTransport, UniversalResolver, UniversalError,
};

let transport = ReqwestTransport::new(rpc_url);
let resolver = UniversalResolver::new(transport, EnsNetwork::Mainnet);
match resolver.resolve_all("sbo3lagent.eth") {
    Ok(records) => println!("{records:?}"),
    Err(UniversalError::OffchainResolverRequiresCcipFlow) => {
        // fall back to LiveEnsResolver here
    }
    Err(e) => return Err(e.into()),
}
```

The struct also implements `EnsResolver`, so it slots into existing
call sites that take `&dyn EnsResolver` without modification.
