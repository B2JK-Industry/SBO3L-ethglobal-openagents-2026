# T-3-8 — Cross-chain agent identity

**Status:** Shipped (this PR).
**Track:** ENS — Phase 2 deepening.
**Crate:** `crates/sbo3l-identity/src/cross_chain.rs`.

## Why

A single agent identity is provably the same entity across multiple
EVM chains. The same `agent_id` (issued on L1 ENS under
`sbo3lagent.eth`) can attest its presence on Optimism, Base,
Polygon, Arbitrum, Linea — and any verifier can confirm:

1. The attestation was signed by the agent's canonical signing key
   (the one published as `sbo3l:cross_chain_pubkey` on L1).
2. The same `(agent_id, owner, signing_pubkey)` triple appears on
   every chain the agent claims to operate on.
3. Each chain's attestation is bound to that chain's id — no
   replaying a Polygon attestation on Optimism.

## Mechanism

Each chain stores a single ENS text record
`sbo3l:cross_chain_attestation` whose value is the JSON-serialised
`CrossChainAttestation`:

```json
{
  "chain_id": 10,
  "agent_id": "research-agent-01",
  "owner": "0xdc7e0dc7e0dc7e0dc7e0dc7e0dc7e0dc7e0dc7e0",
  "signing_pubkey": "<32-byte hex Ed25519 public key>",
  "issued_at": 1714694400,
  "signature": "<64-byte hex Ed25519 signature>"
}
```

The signature covers the EIP-712 typed-data digest of
`(chain_id, agent_id, owner)` under the SBO3L Cross-Chain Identity
domain.

### EIP-712 domain

```
EIP712Domain(
    string name           = "SBO3L Cross-Chain Identity",
    string version        = "1",
    uint256 chainId       = 1,                       // anchored on mainnet
    address verifyingContract = address(0)           // off-chain attestation
)
```

The domain's `chainId` is anchored to mainnet (`1`) regardless of
where the attestation is submitted — the domain identifies the
*scheme*, the per-attestation `chain_id` field identifies the
*target chain*. Without this split, two attestations for the same
agent on Optimism and Polygon would have different domain
separators and the consistency check would have to special-case
every chain.

### Struct hash

```
CrossChainIdentity(string agent_id, address owner, uint256 chain_id)
```

`agent_id` is hashed as `keccak256(bytes(agent_id))`; `owner` is a
20-byte address left-padded to 32 bytes; `chain_id` is a 32-byte
big-endian uint256.

The 32-byte digest is `keccak256(0x1901 || domainSeparator || structHash)`.

## Verification

Three layers — each adds coverage; callers compose what they need.

| Layer | Function | Coverage |
|---|---|---|
| 1 | `verify_attestation` | sig recovers under embedded pubkey |
| 2 | `verify_attestation_with_context` | sig + chain match + owner match + freshness |
| 3 | `verify_consistency` | sig × N + same `(agent_id, owner, signing_pubkey)` × N + distinct chain ids |

`verify_consistency` returns a `ConsistencyReport` carrying the
canonical identity tuple and the chain-id set the agent is attested
on. Callers compare against an expected set ("I expected
mainnet+optimism+base, I got just mainnet+optimism — agent is
missing a base attestation").

## On-chain submission

`build_set_attestation_calldata(node, attestation)` packs a
`setText("sbo3l:cross_chain_attestation", json)` call ready for
broadcast to any chain's PublicResolver. Same shape as the existing
ENS anchor envelope; same selector.

Committing the consistency proof to the audit chain:
`commit_report(report)` — JCS+SHA-256 over the canonical
`ConsistencyReport`. Pin the commitment in a receipt, the
underlying report can be re-fetched and re-verified later.

## Why Ed25519 today, secp256k1 tomorrow

Ed25519 is already a dependency for the cross-agent receipt
signing (`sbo3l-core` + `cross_agent`). Adding secp256k1 to this PR
would balloon the dep tree for a verification path that an
Ethereum smart contract can't run yet anyway (no live deploy, no
caller).

When **F-5 EthSigner** lands the Ethereum-native signer trait, this
module gains a parallel `EcdsaCrossChainVerifier` that verifies via
`ecrecover` — both verifiers consume the SAME EIP-712 digest, so
the on-chain transition is a signing-side swap, not a wire-format
break.

## Tests

26 unit tests — all passing under `cargo test -p sbo3l-identity --lib`.

| Test | Asserts |
|---|---|
| `eip712_digest_is_deterministic` | digest = digest |
| `eip712_digest_changes_with_chain_id` | replay protection |
| `eip712_digest_changes_with_agent_id` | binding |
| `eip712_digest_changes_with_owner` | binding |
| `sign_then_verify_round_trip` | crypto layer works |
| `tampered_signature_rejected` | bit-flip detected |
| `tampered_chain_id_rejected` | post-sign tamper detected |
| `cross_chain_consistency_happy_path` | 4-chain attestation set verifies |
| `consistency_rejects_owner_drift` | inconsistent owner caught |
| `consistency_rejects_agent_id_drift` | inconsistent id caught |
| `consistency_rejects_pubkey_drift` | inconsistent key caught |
| `consistency_rejects_duplicate_chain` | two attestations for same chain rejected |
| `consistency_rejects_empty_set` | empty input rejected |
| `verify_with_context_chain_mismatch` | wrong-chain attestation caught |
| `verify_with_context_owner_mismatch` | wrong-owner caught |
| `verify_with_context_owner_case_insensitive` | EIP-55 mixed case OK |
| `verify_with_context_freshness` | stale attestations rejected |
| `json_round_trip` | wire format stable |
| `json_rejects_unknown_fields` | strict-mode JSON |
| `calldata_for_set_attestation_starts_with_set_text_selector` | on-chain submission shape correct |
| `commit_report_is_deterministic` | audit-anchor commitment stable |
| `commit_report_changes_when_inputs_change` | commitment is bound to inputs |
| `malformed_owner_hex_rejected` | input validation |
| `malformed_pubkey_hex_rejected` | input validation |
| `domain_type_hash_pinned` | type-string drift guard |
| `known_chain_id_round_trip` | enum ↔ id complete |

## Public surface

```rust
use sbo3l_identity::{
    sign_attestation, verify_consistency, KnownChain, CrossChainAttestation,
};

let attestations = vec![
    sign_attestation(&key, KnownChain::Mainnet.id(), "research-agent-01", &owner, now),
    sign_attestation(&key, KnownChain::Optimism.id(), "research-agent-01", &owner, now),
    sign_attestation(&key, KnownChain::Base.id(), "research-agent-01", &owner, now),
];

let report = verify_consistency(&attestations)?;
println!("agent {} consistent across chains: {:?}", report.agent_id, report.chains);
```

## Future work

- secp256k1/ecrecover verifier (gated on F-5 EthSigner landing).
- L1 → L2 light-client / ZK proof of L1 ENS state, so an L2
  contract can verify the canonical attestation without trusting
  off-chain SBO3L tooling. The wire format already supports this
  upgrade — only the verifier changes.
- CLI subcommand `sbo3l agent attest <chain> --owner <addr>` that
  reads the agent's signing key, builds + prints the attestation
  in dry-run, optionally broadcasts under a chain-specific RPC.
