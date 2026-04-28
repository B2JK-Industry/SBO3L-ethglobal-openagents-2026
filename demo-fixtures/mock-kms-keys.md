# `mock-kms-keys.json` — production-shaped mock-KMS key catalogue

Public verification-key metadata for Mandate's two demo signers — same
deterministic dev seeds that ship in `crates/mandate-server/src/lib.rs:54-55`
and that the production-shaped runner's step 9
(`demo-scripts/run-production-shaped-mock.sh`) uses to verify audit
bundles. **Public Ed25519 verification keys only — no private/signing
material is included.**

## What it demonstrates

- The catalogue shape that a future `mandate key list --mock` CLI
  (PSM-A1.9) would emit. Adapter authors can target a stable JSON shape
  now without waiting for the CLI to land.
- Per-key metadata fields that production callers need: `key_id`,
  `key_version`, `algorithm`, `purpose` (`audit_event_signing` /
  `policy_receipt_signing`), `verifying_key_hex`, `created_at_iso`,
  `rotated_at_iso`, `active`, plus `mock: true` and a
  `production_warning` per entry.
- An explicit `seed_source` field per key documenting the **public**
  derivation (`DevSigner::from_seed("audit-signer-v1", [11u8; 32])`).
  These are not secrets — the seed bytes are committed in the open
  repo. The fixture documents the path so future maintainers can
  recompute the verification keys mechanically.

## What live system it stands in for

A real KMS / HSM key-listing API output:

- AWS KMS — `kms:ListKeys` + `kms:GetPublicKey` for each key id.
- GCP KMS — `KeyManagementService.ListCryptoKeys`.
- Azure Key Vault — `KeyClient.list_properties_of_keys()`.
- Self-hosted HSM — vendor-specific key-listing API.

Production deployments inject signers via `AppState::with_signers`
(TEE/HSM-backed). The dev signers in `mandate-server::lib.rs` are
clearly labelled `⚠ DEV ONLY ⚠` and never ship as production keys.

## Exact replacement step

This is a two-stage replacement. **Stage 1** (PSM-A1.9) introduces a
mock-KMS CLI surface that emits this fixture's shape from a real local
keyring. **Stage 2** (production) swaps the mock keyring for a real
KMS / HSM client.

### Stage 1 — `mandate key list --mock` CLI (PSM-A1.9)

1. Land Developer A's PSM-A1.9 PR (`feat: persist mock kms keyring + add mandate key cli`).
2. Output of `mandate key list --mock --format json` should match this
   fixture's shape (same fields, same envelope).
3. Update `demo-scripts/run-production-shaped-mock.sh` step 9 to read
   the audit/receipt verification pubkeys from
   `mandate key list --mock --format json` instead of the current
   hardcoded constants. Comment in the script already points at this
   transition (`crates/mandate-server/src/lib.rs:54-55`).
4. The fixture stays as the **shape contract** for the CLI's JSON
   output — the validator (`test_fixtures.py`) protects against drift.

### Stage 2 — production KMS / HSM

1. Implement a `Signer` trait variant that calls the chosen KMS / HSM:
   - AWS KMS — sign via `kms:Sign` with the key id from `key_list`.
   - HSM — vendor SDK `signer.sign(...)`.
2. Configure via env vars (canonical names — see
   [`docs/production-transition-checklist.md` §Signer](../docs/production-transition-checklist.md#signer--mock-kms--hsm)):
   - `MANDATE_SIGNER_BACKEND` — `dev` | `mock_kms` | `aws_kms` | `hsm`.
   - `MANDATE_AUDIT_SIGNER_KEY_ID` — KMS key id for the audit signer.
   - `MANDATE_RECEIPT_SIGNER_KEY_ID` — KMS key id for the receipt signer.
   - `MANDATE_KMS_REGION` / `MANDATE_KMS_ENDPOINT` for the KMS API
     (vendor-specific equivalents apply for non-AWS backends).
3. Construct `AppState::with_signers(...)` from the configured backend
   instead of the dev signers. Existing `AppState::new()` continues to
   use deterministic dev seeds and stays clearly labelled `⚠ DEV ONLY ⚠`.
4. The `verifying_key_hex` for production keys must NEVER be one of the
   two values in this fixture — those are the public dev pubkeys and
   would represent a catastrophic key-management failure if ever seen
   in production output.

See
[`docs/production-transition-checklist.md` §Mock KMS / HSM signer](../docs/production-transition-checklist.md#signer--mock-kms--hsm)
for the env-var / endpoint / credentials matrix.

## Truthfulness invariants

- The fixture sets `no_private_material: true` and the validator
  enforces that no `signing_key_hex` / `private_key_hex` / `seed_hex` /
  `seed_bytes_hex` field with ≥ 32 hex chars appears anywhere.
- Every key entry has a `production_warning` field stating the seed is
  DEV ONLY.
- The two `verifying_key_hex` values are deterministically reproducible
  from the seeds named in `seed_source` — anyone can compute them
  offline and confirm. Re-derivation procedure: feed the named 32-byte
  seed into Ed25519's `secret_to_public` (e.g. Python's `cryptography`
  package: `Ed25519PrivateKey.from_private_bytes(bytes([11]*32)).public_key().public_bytes(Raw, Raw).hex()`).
- The fixture's envelope (`mock: true`, `schema`, `explanation`,
  `live_replacement`) is enforced by
  [`test_fixtures.py`](test_fixtures.py).

## Where this fixture is referenced

- [`README.md`](README.md) §B3 fixtures
- [`test_fixtures.py`](test_fixtures.py) (validator + `no_private_material` guard)
- [`../docs/production-transition-checklist.md` §Signer](../docs/production-transition-checklist.md#signer--mock-kms--hsm)
- The two `verifying_key_hex` values match the constants in
  `demo-scripts/run-production-shaped-mock.sh` step 9 (production-shaped
  runner — verifies audit bundles using these public keys).
- The seed sources match `crates/mandate-server/src/lib.rs:54-55`.
