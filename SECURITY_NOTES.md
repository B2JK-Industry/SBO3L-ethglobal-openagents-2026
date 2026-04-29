# Security Notes — SBO3L ETHGlobal Open Agents 2026

## Scope

This is a hackathon-scope demo. SBO3L's daemon and CLI are labelled `⚠ DEV ONLY ⚠` in code (`AppState::new()` warning at [`crates/sbo3l-server/src/lib.rs`](crates/sbo3l-server/src/lib.rs), the dev signer comments in [`crates/sbo3l-core/src/signer.rs`](crates/sbo3l-core/src/signer.rs), and every sponsor-adapter `local_mock()` constructor in [`crates/sbo3l-execution/`](crates/sbo3l-execution/)). The notes below pin specific known limitations a production deployment would need to address. Each item is honest disclosure, not a roadmap promise: nothing here is committed to ship within this hackathon.

## Known limitations (scope-cut for submission)

### Daemon authentication
`POST /v1/payment-requests` has no auth middleware in this build; `agent_id` is trusted from the request JSON. The daemon binds to `127.0.0.1` by default and is documented as DEV ONLY. Production path: mTLS or JWT capability tokens cryptographically bound to `agent_id`, plus a refuse-non-loopback guard on startup unless an auth backend is configured. Tracked as post-hackathon work.

### Production signer wiring
`sbo3l-server` constructs `AppState::new()` directly today, which uses the deterministic public dev seed. `AppState::new()` is documented `⚠ DEV ONLY ⚠`. Production path: `AppState::with_signers(...)` injects a real KMS-backed `SignerBackend`; daemon refuses startup if `SBO3L_SIGNER_BACKEND` is unset. Mock-KMS persistence (PSM-A1.9, V005) is the production-shaped lifecycle preview, not the production signer. Tracked.

### Budget tracker persistence
Budget caps (`per_tx`, `daily`, `monthly`, `per_provider`) are an in-memory `HashMap` inside `AppState`. They reset on daemon restart; they don't survive multi-process deployment. Production path: SQLite-backed budget rows committed transactionally alongside the nonce-replay claim and the audit append. Tracked.

### Idempotency in-flight semantics
`Idempotency-Key` cache lookup happens before the pipeline runs; cache write happens after the pipeline returns 200. Concurrent same-key requests can race and both pass the lookup. Production path: a `processing` / `succeeded` / `failed` state machine with an atomic reservation on first lookup, so a second concurrent request blocks or returns 409 instead of running the pipeline twice. Tracked.

### Passport verifier scope
`sbo3l passport verify` is **structural-only by design** (schema + cross-field invariants — see the doc-comment at [`crates/sbo3l-cli/src/passport.rs`](crates/sbo3l-cli/src/passport.rs) line 11). It does **not** verify Ed25519 signatures, audit-chain hash linkage, or recompute the canonical APRP / policy hashes. Cryptographic verification lives in `sbo3l audit verify-bundle`, which the capsule references via `audit.bundle_ref`. A future Passport v2 verifier can wrap `sbo3l-core::audit_bundle::verify` + receipt signature verification under one CLI; for this build, the structural verifier is the explicit scope and the capsule's `verification.offline_verifiable` field reflects the structural result, not a full crypto re-verify.

## What is real today

For the positive-side reality (what is implemented + reproducible), see [`README.md` §Status / §What is real vs mocked](README.md) and [`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md). Both are authoritative; this file is the negative-side complement.
