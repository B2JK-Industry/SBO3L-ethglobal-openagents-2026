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

## Known limitations (scope-cut for submission)

### Production signer wiring
`sbo3l-server` constructs `AppState::new()` directly today, which uses the deterministic public dev seed. `AppState::new()` is documented `⚠ DEV ONLY ⚠`. Production path: `AppState::with_signers(...)` injects a real KMS-backed `SignerBackend`; daemon refuses startup if `SBO3L_SIGNER_BACKEND` is unset. Mock-KMS persistence (PSM-A1.9, V005) is the production-shaped lifecycle preview, not the production signer. Tracked.

### Idempotency in-flight semantics
`Idempotency-Key` cache lookup happens before the pipeline runs; cache write happens after the pipeline returns 200. Concurrent same-key requests can race and both pass the lookup. Production path: a `processing` / `succeeded` / `failed` state machine with an atomic reservation on first lookup, so a second concurrent request blocks or returns 409 instead of running the pipeline twice. Tracked.

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
