# Mock KMS signer

> *Production-shaped, not production-ready.*

`MockKmsSigner` (in `crates/mandate-core/src/mock_kms.rs`) is a local Ed25519 signer that **mimics the lifecycle shape of a managed KMS** — versioned keys, stable role names, rotation, historical-key verification — without ever leaving the process or talking to a real KMS.

## What it is, and what it is not

| | Mock KMS (this module) | A real KMS / HSM |
|---|---|---|
| Key custody | Local process memory; private key derivable from a local root seed | Hardware-isolated; never leaves the device |
| Rotation | `rotate()` advances an in-memory keyring version | Provider-managed; backed by audit + access control |
| Network | None — fully offline, deterministic | API + auth required, with rate limits and SLAs |
| Failure modes | Compile-time / `VerifyError` | Auth failures, unavailable, throttled, attestation mismatches |
| Audit | None | Provider-side audit, plus your own |

**This is not a stepping stone where flipping one flag turns it into a real KMS.** The function `derive_signing_key(role, version, root_seed)` is replaceable, but the trust model around custody, rotation policy, attestation, and recovery is not modelled here. A real implementation would replace the entire backend behind the `SignerBackend` trait and rebuild a different operational story around it.

The deterministic mock IS useful for:

- exercising rotation semantics in tests and demos without flakiness;
- showing reviewers a signer-trait boundary (`SignerBackend`) that production-grade backends can plug into;
- proving that `PolicyReceipt` / `SignedAuditEvent` / `DecisionToken` already verify *across* a key rotation, with the right `key_id` resolution.

## API surface

`mandate_core::signer::SignerBackend` — the trait every signing surface uses:

```rust
pub trait SignerBackend {
    fn current_key_id(&self) -> &str;
    fn sign_hex(&self, message: &[u8]) -> String;
    fn current_public_hex(&self) -> String;
}
```

Both `DevSigner` and `MockKmsSigner` implement it. `UnsignedReceipt::sign`, `SignedAuditEvent::sign`, and `DecisionPayload::sign` all accept `&impl SignerBackend`, so swapping backends is a one-line change at the construction site.

`mandate_core::mock_kms::MockKmsSigner`:

```rust
let mut s = MockKmsSigner::new(
    /* role        */ "audit-mock",
    /* root_seed   */ [42u8; 32],
    /* genesis     */ chrono::Utc::now(),
);
assert_eq!(s.current_key_id(), "audit-mock-v1");

// Sign through the trait.
let sig = SignerBackend::sign_hex(&s, b"some canonical bytes");

// Rotate.
let v2 = s.rotate();
assert_eq!(v2.version, 2);
assert_eq!(v2.key_id, "audit-mock-v2");

// Old signatures stay verifiable via key_id resolution.
let v1 = s.key_by_id("audit-mock-v1").unwrap();
mandate_core::signer::verify_hex(&v1.public_hex, b"some canonical bytes", &sig).unwrap();
```

`MockKmsKeyMeta` exposes:

- `role`, `version`, `key_id`, `public_hex`, `created_at`,
- a `mock: bool` field that is **always `true`** so JSON / CLI output cannot drop the disclosure.

## CLI

The CLI commands `mandate key list --mock` and `mandate key rotate --mock` are tracked separately. They require a small storage table to persist rotation state across CLI invocations; see the production-shaped mock backlog (item PSM-A1.9) for follow-up.

For now, callers use `MockKmsSigner` programmatically (tests, in-process daemons, fixtures).

## Tests

`crates/mandate-core/src/mock_kms.rs::tests` covers:

- v1 keyring metadata is deterministic;
- different root seeds yield different keys;
- `SignerBackend` round-trip;
- `current_key_id()` / `current_public_hex()` agree with the keyring;
- `rotate()` advances the version and changes the public key;
- pre-rotation signatures still verify via resolved historical pubkey;
- post-rotation pubkey rejects pre-rotation signatures;
- unknown `key_id` → `VerifyError::BadPublicKey`;
- `from_versions(N)` reconstructs the same keyring as `new(...) + N-1 rotations`;
- `current_as_dev_signer()` produces compatible signatures (compat shim);
- end-to-end: receipt / audit event / decision token sign + verify through `MockKmsSigner`;
- end-to-end: a pre-rotation receipt verifies under the resolved v1 pubkey after the keyring rotates to v2; a v2 pubkey rejects the v1 receipt.

## Limitations carried forward (truthful disclosure)

- No persistence of rotation state across processes (programmatic only).
- No CLI surface yet — see PSM-A1.9.
- No live KMS / HSM backend implements `SignerBackend` in this build.
- `derive_signing_key` is a deterministic seed-stretch, **not** a production KDF; do not adopt it for any non-mock context.
- The daemon (`mandate-server`) still constructs `DevSigner` for receipts and audit events; wiring the daemon to use `MockKmsSigner` (and persisting rotation state) is also future work.
