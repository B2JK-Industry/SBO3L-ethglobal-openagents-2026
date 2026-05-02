# Changelog — `@sbo3l/marketplace`

All notable changes to this package follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.2.0] — 2026-05-02

### Added

- Initial release. Content-addressed, signed policy registry SDK.
- `computePolicyId(policy)` — derives stable `sha256-<hex>` id from RFC 8785 (JCS) canonical JSON.
- `signBundle({ policy, issuer_id, issuer_privkey_hex, ... })` — Ed25519-signs the canonical bytes; returns a complete `SignedPolicyBundle`.
- `IssuerRegistry` — trusted-issuer key lookup table; consumers seed from config / on-chain / curated allowlist.
- `verifyBundle(bundle, registry)` — checks all 4 invariants: metadata present, policy_id matches content hash, signature valid, issuer trusted with matching pubkey. Returns `{ ok: true, policy } | { ok: false, code, detail }` with stable codes (`policy_id_mismatch`, `signature_invalid`, `issuer_unknown`, `issuer_pubkey_mismatch`, `metadata_missing`).
- `MarketplaceTransport` interface + `InMemoryTransport` (tests + examples) + `HttpTransport` (reference implementation for hosted registries).
- `publishPolicy(transport, bundle)` — refuses to store a bundle whose `policy_id` doesn't match its content hash (prevents poisoning).
- `fetchAndVerifyPolicy(transport, registry, id)` — one-shot fetch + verify with structured result.
- `bootstrapOfficialRegistry(officialPubkeyHex)` — returns an `IssuerRegistry` pre-trusting the SBO3L official issuer.
- Subpath `@sbo3l/marketplace/policies` — 3 starter `SignedPolicyBundle` fixtures (low-risk research, medium-risk trading, high-risk treasury), each issued by a different issuer to demonstrate the registry pattern.
- 30 vitest tests covering canonical JSON determinism, content-hash derivation, sign + verify round-trip, all 5 verify failure modes, transport round-trip, HTTP transport URL shape, starter bundle integrity.

### Dependencies

- `@noble/ed25519` ^2.1.0
- `@noble/hashes` ^1.4.0

### Peer dependencies

- `@sbo3l/sdk` ^1.0.0 || ^1.2.0 (optional)

[1.2.0]: https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/releases/tag/marketplace-v1.2.0
