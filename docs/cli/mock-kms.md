# Mock KMS signer

> *Production-shaped, not production-ready.*

`MockKmsSigner` (in `crates/sbo3l-core/src/mock_kms.rs`) is a local Ed25519 signer that **mimics the lifecycle shape of a managed KMS** — versioned keys, stable role names, rotation, historical-key verification — without ever leaving the process or talking to a real KMS.

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

`sbo3l_core::signer::SignerBackend` — the trait every signing surface uses:

```rust
pub trait SignerBackend {
    fn current_key_id(&self) -> &str;
    fn sign_hex(&self, message: &[u8]) -> String;
    fn current_public_hex(&self) -> String;
}
```

Both `DevSigner` and `MockKmsSigner` implement it. `UnsignedReceipt::sign`, `SignedAuditEvent::sign`, and `DecisionPayload::sign` all accept `&impl SignerBackend`, so swapping backends is a one-line change at the construction site.

`sbo3l_core::mock_kms::MockKmsSigner`:

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
sbo3l_core::signer::verify_hex(&v1.public_hex, b"some canonical bytes", &sig).unwrap();
```

`MockKmsKeyMeta` exposes:

- `role`, `version`, `key_id`, `public_hex`, `created_at`,
- a `mock: bool` field that is **always `true`** so JSON / CLI output cannot drop the disclosure.

## CLI (PSM-A1.9)

`sbo3l key {init,list,rotate}` operate on the persistent `mock_kms_keys` SQLite table (migration **V005**). Every operation requires `--mock` for explicit disclosure — these commands are NOT plug-compatible with a production KMS.

```bash
# Initialise role=audit-mock at v1, deterministic seed, in-process db.
sbo3l key init --mock \
  --role      audit-mock \
  --root-seed 2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a \
  --db        /var/lib/sbo3l/sbo3l.sqlite

# List the keyring (filter by --role optional).
sbo3l key list --mock --db /var/lib/sbo3l/sbo3l.sqlite

# Rotate to next version. Reads max(version) for the role, derives v(n+1)
# from (role, n+1, root_seed), inserts the row. Old version stays listed
# so a verifier can resolve historical key_ids back to public keys.
sbo3l key rotate --mock \
  --role      audit-mock \
  --root-seed 2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a \
  --db        /var/lib/sbo3l/sbo3l.sqlite
```

The `--root-seed` is **never** stored in the SQLite database — only the resulting public-key metadata is persisted. The seed is supplied on every operation; rotation requires the same seed used at init (otherwise the derived public key wouldn't match what `MockKmsSigner` would produce).

`init` is idempotent: re-running with the same args reports the existing v1 row and exits 0.

`sbo3l doctor` automatically promotes the `mock_kms_keys` row from `skip` to `ok` once V005 has run and at least one keyring exists.

## Tests

`crates/sbo3l-core/src/mock_kms.rs::tests` covers:

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

## What ships today (PSM-A1 + PSM-A1.9)

- In-process `MockKmsSigner` (PSM-A1) — programmatic API for signing receipts / audit events / decision tokens through the `SignerBackend` trait.
- Persistent mock keyring storage in SQLite (PSM-A1.9, migration V005 `mock_kms_keys`) — public-key metadata only; root seeds are never persisted.
- `sbo3l key {init,list,rotate} --mock --db <path>` CLI — every operation requires `--mock`; every output line is `mock-kms:`-prefixed; rotate refuses on mismatched root-seed; current-version lookup propagates real DB errors (no silent "no keyring" mask).
- `sbo3l doctor` reports `mock_kms_keys` as `ok` once V005 is applied.

## Limitations carried forward (truthful disclosure)

- **`derive_signing_key` is a deterministic seed-stretch, not a production KDF.** Do not adopt it for any non-mock context.
- **No live KMS / HSM backend.** Production deployments inject signers via `AppState::with_signers` (TEE/HSM-backed). The `SBO3L_SIGNER_BACKEND` selector and per-role `SBO3L_*_SIGNER_KEY_ID` env vars are documented in [`docs/production-transition-checklist.md`](../production-transition-checklist.md#signer--mock-kms--hsm); none of those wirings exist in this build.
- **Daemon still constructs `DevSigner`.** `sbo3l-server` does not yet load the persistent mock keyring at startup. Wiring the daemon to read v(n) public keys from the mock-KMS table for receipt/audit signing is follow-up work, not part of PSM-A1.9.
- **`--root-seed` is a CLI input, not a secret.** It is *never* persisted to the SQLite DB (only the per-version public material is). Operators must keep the seed out of shell history / process listings the same way they would treat any other key bootstrap input.
