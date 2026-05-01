# Security Notes — SBO3L ETHGlobal Open Agents 2026

## Scope

This is a hackathon-scope demo. SBO3L's daemon and CLI are labelled `⚠ DEV ONLY ⚠` in code (`AppState::new()` warning at [`crates/sbo3l-server/src/lib.rs`](crates/sbo3l-server/src/lib.rs), the dev signer comments in [`crates/sbo3l-core/src/signer.rs`](crates/sbo3l-core/src/signer.rs), and every sponsor-adapter `local_mock()` constructor in [`crates/sbo3l-execution/`](crates/sbo3l-execution/)). The notes below pin specific known limitations a production deployment would need to address. Each item is honest disclosure, not a roadmap promise: nothing here is committed to ship within this hackathon.

## Daemon authentication (F-1, required by default)

`POST /v1/payment-requests` requires authentication. Two acceptable forms of `Authorization: Bearer <token>` ([`crates/sbo3l-server/src/auth.rs`](crates/sbo3l-server/src/auth.rs)):

1. **Plain bearer** — bcrypt-verified against the hash held in env `SBO3L_BEARER_TOKEN_HASH` (htpasswd-shaped, e.g. `$2y$05$...`).
2. **JWT (EdDSA)** — verified against the Ed25519 public key in env `SBO3L_JWT_PUBKEY_HEX` (64 hex chars). The JWT `sub` claim must equal the APRP `agent_id`, otherwise the request is rejected with `auth.agent_id_mismatch`.

Default-deny: a request without `Authorization` is rejected with HTTP 401 + `auth.required` (RFC 7807). The development-only bypass `SBO3L_ALLOW_UNAUTHENTICATED=1` is advertised at startup with a stderr `⚠ UNAUTHENTICATED MODE — DEV ONLY ⚠` banner; never enable in production. The auth gate runs before idempotency, before the nonce gate, and before any signing — a rejected request produces zero side effects.

## Budget persistence (F-2, ACID across restart)

Budget caps (`per_tx`, `daily`, `monthly`, `per_provider`) persist in SQLite via the V008 `budget_state` table ([`crates/sbo3l-storage/src/budget_store.rs`](crates/sbo3l-storage/src/budget_store.rs)). On the request path, [`BudgetTracker::commit`](crates/sbo3l-policy/src/budget.rs) wraps the budget upsert AND the audit append in a single rusqlite transaction via `Storage::finalize_decision`: both writes either land or roll back together. Daemon restart against the same SQLite file replays committed spend; deny on restart with the spec-canonical `policy.budget_exceeded` is exercised by `cargo test --test test_budget_persistence`.

`per_tx` is a single-request cap and is intentionally never persisted — its row is never written, only the request amount is compared.

## Idempotency atomicity (F-3, race-safe state machine)

The HTTP daemon CLAIMs an `Idempotency-Key` atomically before running the pipeline. V009 adds a `state TEXT NOT NULL CHECK (state IN ('processing','succeeded','failed'))` column to `idempotency_keys`; the request path uses three new race-safe primitives in [`crates/sbo3l-storage/src/idempotency_store.rs`](crates/sbo3l-storage/src/idempotency_store.rs):

* `Storage::idempotency_try_claim` — atomic `INSERT … state='processing'`. PRIMARY KEY collision returns the existing row instead of running the pipeline.
* `Storage::idempotency_succeed` / `idempotency_fail` — UPDATE the row to its terminal state once the pipeline returns. Only fire on the `processing → succeeded|failed` edge.
* `Storage::idempotency_try_reclaim_failed` — atomic `UPDATE … WHERE state='failed' AND created_at < cutoff`. Past the 60-second grace window exactly one concurrent reclaimer wins; others see rows = 0 and surface `idempotency_in_flight`.

Behaviour matrix:

| Pre-claim observed row | Same body | Outcome |
|---|---|---|
| (none) | — | claim wins, pipeline runs, finalize at end |
| `succeeded` | yes | byte-identical cached replay (no pipeline run) |
| `succeeded` | no | HTTP 409 `protocol.idempotency_conflict` |
| `processing` | (any) | HTTP 409 `protocol.idempotency_in_flight` |
| `failed` (within 60s) | (any) | HTTP 409 `protocol.idempotency_in_flight` |
| `failed` (past 60s) | — | reclaim wins → pipeline runs; reclaim race losers get `idempotency_in_flight` |

The 50-concurrent stress is in `cargo test --test test_idempotency_race`. Pre-F-3 the lookup-then-INSERT race let multiple writers run the pipeline; F-3's atomic claim caps that at exactly one pipeline run per `Idempotency-Key`.

## Signer backends (F-5, KMS abstraction)

The signing surface is a [`Signer`] trait in [`crates/sbo3l-core/src/signers/mod.rs`](crates/sbo3l-core/src/signers/mod.rs) with four backends selectable via the `SBO3L_SIGNER_BACKEND` env var:

| Value | Feature flag | Status |
|---|---|---|
| `dev` (default) | always on | Production-mode-locked dev signer. Refuses to construct unless `SBO3L_DEV_ONLY_SIGNER=1` is set; on construction prints a `⚠ DEV ONLY SIGNER ⚠` stderr banner. The seeds are public constants in this repo — anyone can forge a signature that passes `verify_hex` against this backend. |
| `aws_kms` | `aws_kms` | AWS KMS Ed25519 signer. **Compile-only stub today**; SDK wiring lands in a follow-up nightly task once Daniel provisions a KMS test key. Calls return `SignerError::Kms(...)` until then. |
| `gcp_kms` | `gcp_kms` | Google Cloud KMS Ed25519 signer. Same status as AWS. |
| `phala_tee` | `phala_tee` | **Phase 3 placeholder**, NOT a real TEE today. Locks the trait shape so the rest of the codebase can plumb through `Box<dyn Signer>` without churn when the real TEE wiring lands. |

The daemon's startup path validates the configured backend at boot via `signer_from_env("audit")` + `signer_from_env("receipt")`; any error (DevOnlyLockout, BackendNotCompiled, MissingEnv, UnknownBackend) prints a stderr diagnostic and exits with code 2.

Sibling secp256k1 trait [`EthSigner`] in [`crates/sbo3l-core/src/signers/eth.rs`](crates/sbo3l-core/src/signers/eth.rs) is a feature-flagged stub for Dev 4's EVM transaction signing (Durin subname issuance). Deliberately **not** merged with [`Signer`] because the curves and wire formats differ (Ed25519 64-byte vs secp256k1 65-byte `r||s||v`). Both can be backed by the same KMS with different key shapes.

All four Ed25519 backends produce **identical wire format**: 64-byte signatures verifiable via `crate::signer::verify_hex` against the 32-byte verifying key the backend reports. A receipt signed by `AwsKmsSigner` is byte-equivalent to one signed by `DevSignerLockedDown` — offline verifiers don't need to know which backend produced it. The dev-signer interop test pins this via `cargo test --test test_signers`; the AWS / GCP equivalents land with the live SDK wiring.


## Passport capsule v2 (F-6, self-contained verification)

Capsule schema [`sbo3l.passport_capsule.v2`](schemas/sbo3l.passport_capsule.v2.json) is **additive on v1**: same shape plus two optional embedded fields. When both are present, `passport verify --strict` runs all 6 cryptographic checks WITHOUT auxiliary inputs — no `--policy`, no `--audit-bundle`, no `--receipt-pubkey`.

| Embedded field | Replaces aux input | Strict checks unblocked |
|---|---|---|
| `policy.policy_snapshot` (canonical Policy JSON) | `--policy <path>` | `policy_hash_recompute` |
| `audit.audit_segment` (`sbo3l.audit_bundle.v1`) | `--audit-bundle <path>` + `--receipt-pubkey <hex>` | `receipt_signature`, `audit_chain`, `audit_event_link` |

`audit_segment` is capped at 1 MiB (anti-DoS) — a capsule that exceeds the cap is rejected with `capsule.audit_segment_too_large` before any chain walk.

`passport run` emits v2 by default; `--schema-version v1` forces the legacy shape. `passport explain` prints `verifier-mode: self-contained` for v2 capsules with both embedded fields, `verifier-mode: aux-required` otherwise. v1 capsules continue to verify under their own schema (no regression).

Test fixtures: 5 golden v2 (`v2_golden_001..005`) + 4 tampered v2 covering the v2-specific rejection paths (`v2_tampered_001..004`).

## Known limitations (scope-cut for submission)

### Live KMS integration tests
The AWS and GCP KMS backends ship as compile-only stubs in this build. Live integration tests against real KMS test keys are gated behind the `aws_kms` / `gcp_kms` features and the nightly CI matrix; they don't run on the hot per-PR path. Tracked: Daniel provisions test keys (one-time, ~30 min each), the SDK wiring follows in a per-cloud PR.

### Phala TEE (Phase 3)
The `phala_tee` backend is a placeholder, NOT a real TEE today. Real wiring requires a Phase 3 ticket that pulls the `dstack` runtime, sets up remote attestation, and validates the enclave measurement before trusting a signature. Tracked.

### Passport verifier scope
`sbo3l passport verify` defaults to a **structural-only** pass for backwards compat (schema + cross-field invariants — see the doc-comment at [`crates/sbo3l-cli/src/passport.rs`](crates/sbo3l-cli/src/passport.rs) line 11). The default mode does **not** verify Ed25519 signatures, audit-chain hash linkage, or recompute the canonical APRP / policy hashes. The capsule's `verification.offline_verifiable` field reflects the structural result, not a full crypto re-verify.

**Opt-in cryptographic strict mode (B1):** pass `--strict` (alias `--verify-cryptographically`) to additionally run, in one CLI invocation, a structured 6-check report:

1. **structural** — same as the default mode.
2. **request_hash_recompute** — recompute `request_hash` from `capsule.request.aprp` via JCS+SHA-256 and assert it matches both `capsule.request.request_hash` AND `capsule.decision.receipt.request_hash`. The capsule alone is enough; no auxiliary input required.
3. **policy_hash_recompute** — when `--policy <path>` is supplied, recompute JCS+SHA-256 over the canonical policy snapshot and assert it matches `capsule.policy.policy_hash`.
4. **receipt_signature** — when `--receipt-pubkey <hex>` is supplied, verify the embedded `decision.receipt`'s Ed25519 signature against the canonical receipt body.
5. **audit_chain** — when `--audit-bundle <path>` is supplied, run `sbo3l-core::audit_bundle::verify` over the bundle (signatures + chain linkage + summary consistency).
6. **audit_event_link** — when `--audit-bundle <path>` is supplied, assert that `bundle.summary.audit_event_id == capsule.audit.audit_event_id` and that the bundle's chain segment actually contains that event.

Each crypto check whose auxiliary input is absent is reported as `Skipped(reason)` rather than failed — never a fake-OK. A run where no check failed (`is_ok() == true`) but some were skipped is reported as `PASSED (with skips)` so a reader can't mistake a partial pass for a complete one. A structural failure short-circuits every downstream crypto check (every other entry becomes `Skipped(structural failed; crypto checks not meaningful)`) so the operator knows the structural cause is what to fix first.

**Heads-up on the on-main golden fixture:** `test-corpus/passport/golden_001_allow_keeperhub_mock.json` was built for structural-only coverage and uses placeholder hash values (e.g. `c0bd2fab1234…` instead of the real JCS+SHA-256 of its embedded APRP). Running `sbo3l passport verify --strict --path test-corpus/passport/golden_001_*.json` correctly reports `request_hash_recompute: FAILED` — that is the strict verifier doing its job, not a regression. To exercise strict mode against a cryptographically-valid capsule, use the runtime artifact `demo-scripts/artifacts/passport-allow.json` emitted by `bash demo-scripts/run-production-shaped-mock.sh`.

## What is real today

For the positive-side reality (what is implemented + reproducible), see [`README.md` §Status / §What is real vs mocked](README.md) and [`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md). Both are authoritative; this file is the negative-side complement.
