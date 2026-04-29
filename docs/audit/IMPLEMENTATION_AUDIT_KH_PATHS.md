# Implementation audit — KeeperHub IP-1..IP-5 adoption-readiness

> **Audit performed at main = `65d2d12` (pre-PR #46 merge).** PR #46 (Passport P3.1 — functional `mandate-mcp` stdio server) has since merged to main and resolves the IP-3 surface gap noted in §IP-3 below; IP-1, IP-2, IP-4, IP-5 findings remain valid against current main.

**Audit branch:** `docs/kh-implementation-audit` (uncommitted, working tree only).
**Audited against `main` HEAD:** `65d2d12` (`docs: respond to KeeperHub office-hours feedback (IP-1..IP-5 catalogue) (#45)`).
**Auditor role:** Developer B, read-only.
**Scope:** `crates/`, `migrations/`, `schemas/` for the KeeperHub adapter, audit-bundle codec, Passport capsule + verifier, and MCP surface. Out of scope: trust-badge / operator-console (Python), demo-scripts (shell), `docs/spec/`, ENS / Uniswap adapters except where they share types with KeeperHub.

## Executive summary

- **IP-1 (envelope fields)** is structurally one helper away from adoption-credible. Every `mandate_*` field is already constructible from the existing `PolicyReceipt` + `Policy::canonical_hash()` + `mandate-core::hashing::request_hash()`; today no public helper composes them, but the smallest change is a ~30-line pure-read function on `PolicyReceipt`. **Effort: XS.**
- **IP-4 (standalone adapter crate)** is the highest-leverage move. `mandate-execution` already depends only on `mandate-core` (verified via `cargo metadata` and `cargo tree`); `keeperhub.rs` imports nothing from policy/storage/server/identity/mcp. The only blocker to extraction is that `GuardedExecutor`, `ExecutionReceipt`, `ExecutionError` live in `mandate-execution/src/lib.rs` next to the Uniswap adapter — moving those three types into `mandate-core` (with re-exports) makes the keeperhub adapter trivially extractable. **Effort: S.**
- **IP-5 (Passport capsule)** is fully producible end-to-end from a real Allow decision today on `main`. `mandate passport run` (mandate-cli/src/passport.rs:439) drives the same `POST /v1/payment-requests` pipeline in-process, reads back the audit event, builds a checkpoint, composes a capsule, and self-verifies before write. **No gap; ready.**
- **IP-2 (submission JSON Schema)** is doc-only on the KeeperHub side; on the Mandate side, dropping a `schemas/keeperhub_workflow_submission_v1.json` and wiring it into `scripts/validate_schemas.py` is ~80 lines and demonstrates the contract we'd validate. **Effort: S.**
- **IP-3 (MCP tool)** *was* the largest gap at audit time (`crates/mandate-mcp/src/lib.rs` was literally one comment line — no transport, no tool registration, no handlers). **Resolved by PR #46** (functional `mandate-mcp` stdio JSON-RPC server, Passport P3.1) — see foreword above. The findings below describe the pre-#46 state for the audit trail; the post-#46 surface is documented at [`docs/cli/mcp.md`](../cli/mcp.md) and [`docs/mcp-integration-guide.md`](../mcp-integration-guide.md).

**Single highest-leverage change**: split off `GuardedExecutor` + `ExecutionReceipt` + `ExecutionError` into `mandate-core` (one ~80-line move + re-exports). That single refactor unlocks both IP-4 (clean extraction) and makes the IP-1 envelope helper a natural new module under the future standalone crate. Everything else is additive.

## Truthfulness gaps

`**TRUTHFULNESS GAP**` markers are inline below. Headline gaps:

- `README.md:13` claims **`243/243 green`**. `cargo test --workspace --all-targets` on `main = 65d2d12` reports **263/263**. Stale by 20 tests; the +20 came in PR #44 (Passport CLI MVP). **TRUTHFULNESS GAP**.
- `docs/keeperhub-integration-paths.md:102` (IP-3 schema sketch) regex `^evt-[0-9A-Z]{26}$` is broader than the schema's actual ULID pattern `^evt-[0-7][0-9A-HJKMNP-TV-Z]{25}$` (Crockford base32 with year-prefix). A KeeperHub MCP server validating the loose regex would accept malformed audit-event IDs that Mandate's own schemas reject. **TRUTHFULNESS GAP**, low-impact (regex is in a doc sketch only — but if KeeperHub copy-pastes it, divergence ships).
- `docs/keeperhub-integration-paths.md:188` says the production-shaped runner tally is **"24 real / 0 mock / 1 skipped"**. After PR #44 landed Passport P2.1's step 10b (capsule emit + verify), the tally on current `main` is **`Tally: 26 real, 0 mock, 1 skipped`** (verified by running `bash demo-scripts/run-production-shaped-mock.sh`). Stale by 2 — the +2 is the allow + deny capsule round-trip. **TRUTHFULNESS GAP**, low-impact, partner-facing doc.
- The audit brief itself mentioned `feat/mandate-mcp-passport` as "Dev A's parallel branch in flight". **No such branch exists locally or on origin** as of audit time. `mandate-mcp/` is unchanged from its pre-#42 placeholder state. The brief's premise of a parallel MCP PR is not yet reality. (Reporting per the brief's own audit instruction.)

No code-level overclaims found in the doc layer. The five docs read consistently distinguish "implemented today" from "target".

---

## IP-1 — `mandate_*` envelope fields

### What exists today

Every required field is one struct-field read away from existing types:

- `mandate_request_hash` — `mandate-core::hashing::request_hash(&serde_json::Value) -> Result<String>` (`crates/mandate-core/src/hashing.rs:23`). Public. JCS-canonical SHA-256 of the APRP value.
- `mandate_policy_hash` — `Policy::canonical_hash(&self) -> Result<String, serde_json::Error>` (`crates/mandate-policy/src/model.rs:272`). Public. Hex SHA-256 of canonical policy bytes. Active policies are also stored in V006's `active_policy` SQLite table with the hash pre-computed.
- `mandate_receipt_signature` — `PolicyReceipt.signature.signature_hex: String` (`crates/mandate-core/src/receipt.rs:23,54`). Always populated by `UnsignedReceipt::sign()` (`receipt.rs:82`).
- `mandate_audit_event_id` — `PolicyReceipt.audit_event_id: String` (`receipt.rs:48`). Schema-constrained to `^evt-[0-7][0-9A-HJKMNP-TV-Z]{25}$` (`schemas/policy_receipt_v1.json:62`).
- `mandate_passport_capsule_hash` — **not constructed today**. The Passport capsule artefact is producible (`mandate passport run`, see IP-5), but no helper hashes a capsule's canonical JSON.

### What is missing or misshaped

There is no public helper anywhere in `crates/` that composes the four (target: five) `mandate_*` fields from a `PolicyReceipt`. `rg "mandate_request_hash|mandate_policy_hash|mandate_audit_event_id|mandate_receipt_signature"` against `crates/` returns zero matches. Today a third-party caller would have to hand-construct a JSON object from the right struct fields.

### Smallest change

Add a `KeeperHubEnvelope` (or `MandateUpstreamProof`) struct + `from_receipt(&PolicyReceipt) -> Self` constructor in `crates/mandate-execution/src/keeperhub.rs`:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct KeeperHubEnvelope<'a> {
    pub aprp: &'a serde_json::Value,
    pub policy_receipt: &'a PolicyReceipt,
    pub mandate_request_hash: &'a str,
    pub mandate_policy_hash: &'a str,
    pub mandate_receipt_signature: &'a str,
    pub mandate_audit_event_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mandate_passport_capsule_hash: Option<String>,
}
```

Three reads + one optional hash. ~30 LOC + a unit test that round-trips the envelope to/from JSON.

### Effort

**XS.** Pure additive. No struct changes to PolicyReceipt. Visible from the public crate surface so a downstream `cargo add` consumer sees the helper at the top of `keeperhub.rs`.

### Truthfulness flags

None for IP-1 specifically (the doc claims align with code surface).

---

## IP-2 — submission JSON Schema

### What exists today

Six JSON Schema 2020-12 files under `schemas/` (`aprp_v1`, `policy_v1`, `policy_receipt_v1`, `decision_token_v1`, `audit_event_v1`, `x402_v1`) plus `mandate.passport_capsule.v1.json` and `mandate-mock-{ens-registry,keeperhub-sandbox,kms-keys,uniswap-quotes}-v1`. All registered in `scripts/validate_schemas.py` and validated in CI.

### What is missing or misshaped

No `keeperhub_workflow_submission_v1.json` in `schemas/`. No reference to the IP-1 envelope shape as a JSON Schema we'd consume from KeeperHub.

### Smallest change

Add `schemas/keeperhub_workflow_submission_v1.json` matching the IP-1 envelope shape, plus a one-line registration in `scripts/validate_schemas.py`. The schema would reference `policy_receipt_v1.json` for the `policy_receipt` field via `$ref`. ~80 LOC schema + 1 line in the validator.

This isn't the canonical schema (KeeperHub publishes that); it's the one Mandate would validate **its own outgoing** envelope against, so `KeeperHubExecutor::live()` cannot drift from the agreed shape.

### Effort

**S.** Pure additive. The validator hook is one entry in the existing schema list.

### Truthfulness flags

None.

---

## IP-3 — MCP tool

### What exists today

`crates/mandate-mcp/src/lib.rs` is **one comment line**: `//! Mandate MCP adapter (placeholder).` `Cargo.toml` declares dependencies on `mandate-core`, `serde`, `serde_json`, `thiserror` — that's it. No `tokio`, no `axum`, no JSON-RPC framework, no transport. No tests. No tools registered.

The audit brief mentioned `feat/mandate-mcp-passport` as "Dev A's parallel branch". **No such branch exists locally or on origin** as of audit time (verified via `git branch -a | grep -i mcp`).

### What is missing on main today

For IP-3 specifically (`keeperhub.lookup_execution(execution_id)` plus the symmetric `mandate.audit_lookup(audit_event_id)`):

- **Transport:** none. Stdio JSON-RPC vs HTTP not chosen. The IP-3 doc implies stdio (matches Claude / Cursor MCP conventions).
- **Tool registration:** none. No `Server::new()` / `register_tool()` plumbing. No JSON-RPC dispatch loop.
- **Handlers:** none. No call-site for `mandate audit verify-bundle` or `mandate passport verify` from inside an MCP tool handler.
- **Schema declarations:** none. The IP-3 doc has draft input/output schemas but they are not in any `.json` file or Rust code.
- **Test framework:** no MCP smoke test. `crates/mandate-mcp/tests/` does not exist.

### Smallest change

A skeleton stdio JSON-RPC server that registers `mandate.audit_lookup(audit_event_id)` and dispatches to existing `mandate-core::audit_bundle::build` + `verify` logic. ~250 LOC for transport + tool definition + one handler + one happy-path test. The handler would re-use existing audit-bundle code; no new core feature.

A second tool — `mandate.verify_capsule(capsule_json)` — wraps `mandate-core::passport::verify_capsule` and is even smaller (~20 LOC handler).

### Effort

**M.** First MCP implementation in the workspace. The transport choice (probably hand-rolled stdio JSON-RPC; the Rust MCP SDK is unstable per the live-spike doc's risk note) is the unknown.

### Truthfulness flags

- `docs/keeperhub-integration-paths.md:102` regex `^evt-[0-9A-Z]{26}$` for `mandate_audit_event_id` doesn't match `schemas/audit_event_v1.json:50`'s `^evt-[0-7][0-9A-HJKMNP-TV-Z]{25}$`. **TRUTHFULNESS GAP** (low-impact — sketch in a partner-facing doc).
- `docs/keeperhub-integration-paths.md:108` says "Our MCP tool surface is at `crates/mandate-mcp/` (skeleton today; full implementation tracked in product backlog)". **Accurate** — but a reviewer reading "skeleton" might expect more than one comment line.

---

## IP-4 — standalone adapter crate

### What exists today

`crates/mandate-execution/` contains `keeperhub.rs` + `uniswap.rs` + `lib.rs` (the `GuardedExecutor` trait + `ExecutionReceipt` + `ExecutionError`). `Cargo.toml` declares only:

```toml
mandate-core    = { workspace = true }
serde / serde_json / thiserror / hex / sha2 / ulid / chrono / rust_decimal
```

`cargo metadata` confirms zero workspace dependencies beyond `mandate-core`. `rg "use mandate_(policy|storage|server|identity|mcp)::" crates/mandate-execution/` returns **zero matches**.

`keeperhub.rs` imports only:

```rust
use mandate_core::aprp::PaymentRequest;
use mandate_core::receipt::{Decision, PolicyReceipt};
use crate::{ExecutionError, ExecutionReceipt, GuardedExecutor};
```

`uniswap.rs` is identical except it adds `rust_decimal::Decimal` for swap-policy arithmetic. **The user's claim at PR #45 review (mandate-execution depends only on mandate-core + utility crates) holds at function-signature level for both adapters.**

### What is missing or misshaped

The crate-internal types (`GuardedExecutor` trait, `ExecutionReceipt` struct, `ExecutionError` enum) live in `crates/mandate-execution/src/lib.rs`. To extract `keeperhub.rs` into a standalone `mandate-keeperhub-adapter` crate, those three types either:

1. **Stay in `mandate-execution`** — the new crate would need to depend on `mandate-execution` for the trait, which means a downstream `cargo add mandate-keeperhub-adapter` consumer drags in the Uniswap adapter too. Not ideal.
2. **Move into `mandate-core`** — the new crate depends on `mandate-core` only. Cleanest.
3. **Move into a new `mandate-execution-core` crate** — third option, more crates.

`KeeperHubExecutor::live()` currently has no config struct (`pub fn live() -> Self { Self { mode: KeeperHubMode::Live } }` — `keeperhub.rs:38`). The live-spike doc's target shape (`docs/keeperhub-live-spike.md:71`) is `pub fn live(cfg: KeeperHubLiveConfig) -> Self` with `KeeperHubLiveConfig::from_env()` returning `webhook_url + bearer_token + timeout`. **Documented deviation.** The current shape returns `BackendOffline` from `execute()` regardless, so the no-config form is consistent with "live wiring not landed", but a `KeeperHubLiveConfig` struct (even if `execute_live` is still a stub returning `BackendOffline`) would make the public surface look adoption-ready.

### Smallest change

Phase 1 (XS, ~25 LOC, pure additive): add `pub struct KeeperHubLiveConfig` + `KeeperHubLiveConfig::from_env() -> Result<Self, ExecutionError>` + change `pub fn live()` → `pub fn live(cfg: KeeperHubLiveConfig)`. Body still returns `BackendOffline` from `execute()`. Existing test `live_mode_fails_loudly_without_credentials` (`keeperhub.rs:128`) updates to call `live(KeeperHubLiveConfig { … dummy values … })`.

Phase 2 (S, ~80 LOC moved + re-exports): move `GuardedExecutor`, `ExecutionReceipt`, `ExecutionError` from `mandate-execution/src/lib.rs` into `mandate-core::execution` (new module). Re-export from `mandate-execution::lib` for back-compat. After this, `crates/mandate-keeperhub-adapter/` can be a 1-file crate that depends only on `mandate-core`.

Phase 3 (M, ~120 LOC + crate scaffolding): physically extract `keeperhub.rs` into the new crate, with `Cargo.toml`, `README.md`, an `examples/submit_signed_receipt.rs`, and the IP-1 envelope helper from above. Publishable to crates.io as `mandate-keeperhub-adapter`.

### Effort

Phase 1: **XS**. Phase 2: **S**. Phase 3: **M**. Daniel can choose whether to ship Phase 1+2 (adoption-credible from the public surface) or all three (publishable artefact).

### Cross-crate-coupling inventory for IP-4

| Symbol used in `mandate-execution/src/keeperhub.rs` | Source | Necessary | Notes |
|---|---|---|---|
| `mandate_core::aprp::PaymentRequest` | mandate-core | yes | Trait input. Mandate-core type. |
| `mandate_core::receipt::{Decision, PolicyReceipt}` | mandate-core | yes | Trait input + decision check. |
| `crate::{ExecutionError, ExecutionReceipt, GuardedExecutor}` | mandate-execution | accidental for IP-4 | Should move to mandate-core (Phase 2 above) so the adapter crate can depend on mandate-core only. |
| `ulid::Ulid::new()` | external | yes | execution_ref construction. |
| (test) `mandate_core::receipt::{EmbeddedSignature, ReceiptType, SignatureAlgorithm}` | mandate-core | yes | Test fixture. |

**No leaks** to `mandate-policy`, `mandate-storage`, `mandate-server`, `mandate-identity`, or `mandate-mcp`. The "accidental coupling" is internal to `mandate-execution` and is the only thing standing between today's code and a publishable standalone crate.

### Truthfulness flags

- `docs/keeperhub-integration-paths.md:132` says "no `mandate-policy` / `mandate-storage` / `mandate-server` types leak into the adapter signature". **Verified true** at function-signature level. Doc is accurate.

---

## IP-5 — Passport capsule

### What exists today

Schema, verifier, and CLI all on `main`:

- `schemas/mandate.passport_capsule.v1.json` (PR #42).
- `crates/mandate-core/src/passport.rs` — `verify_capsule(value: &Value) -> Result<(), CapsuleVerifyError>` (PR #42 + #44 fix). 8 cross-field invariants; 1 golden + 8 tampered fixtures under `test-corpus/passport/`.
- `crates/mandate-cli/src/passport.rs` — `cmd_run(args: RunArgs) -> ExitCode` (PR #44, line 439). Drives the existing `POST /v1/payment-requests` pipeline in-process via `oneshot_payment_request`, reads back the audit event, builds a checkpoint, composes a capsule, **self-verifies BEFORE write**, and writes atomically (tempfile + rename).
- `cmd_verify` and `cmd_explain` (PR #44) wrap the verifier with explicit exit codes per `docs/cli/passport.md`.

### End-to-end producibility trace

> POST `/v1/payment-requests` → policy decide → audit append → `mandate audit export` → `mandate passport ...`

This trace **works end-to-end on `main` today**:

1. `mandate-server::router(state)` exposes `POST /v1/payment-requests` → `run_pipeline` (`crates/mandate-server/src/lib.rs:317`).
2. `run_pipeline` validates, hashes the request, claims a nonce, decides, appends an audit event, and signs the receipt. `PaymentRequestResponse.audit_event_id` is a public field (`lib.rs:105`).
3. `mandate audit export --db <path> --receipt <receipt.json> --receipt-pubkey <hex> --audit-pubkey <hex>` (`crates/mandate-cli/src/main.rs:300`) calls `mandate-core::audit_bundle::build()` to package receipt + chain prefix + signer pubkeys into a `mandate.audit_bundle.v1` JSON.
4. `mandate passport run` (`crates/mandate-cli/src/passport.rs:439`) bypasses the export step and goes directly from a fresh APRP → in-process pipeline → capsule, but the **audit-bundle codec is the same `mandate-core::audit_bundle` module**. `mandate.audit_bundle.v1` is referenced from a capsule via `audit.bundle_ref` (capsule schema line 153) — **the bundle itself is not embedded; the field carries a string reference**.
5. `mandate passport verify --path <capsule>` (`crates/mandate-cli/src/passport.rs:43`) runs the full structural verifier including cross-field invariants.

**No gap in producibility.** Capsules are produced + self-verified + atomically written today, mock-mode only. Live-mode (`--mode live`) is rejected at the CLI boundary with a clear message ("live claims require real evidence"; passport.rs:444).

### Smallest change for IP-5 specifically

For IP-5 (KeeperHub stores a `mandate_passport_uri` column pointing at the capsule), the Mandate side needs:

- A way to compute `mandate_passport_capsule_hash` (the IP-1 envelope target field). Two new public helpers in `mandate-core::passport`: `canonical_capsule_bytes(&Value) -> Result<Vec<u8>>` (JCS-canonical) and `capsule_hash(&Value) -> Result<String>` (hex SHA-256 of those bytes). ~25 LOC + a hash-stability test.

That's it. The capsule URI itself is operator-side metadata (where they host the file); Mandate doesn't need to choose the storage scheme.

### Effort

**XS** for the canonical-bytes + hash helper. **No new core features needed for IP-5 producibility** — only the capsule-hash helper that IP-1 also wants.

### Truthfulness flags

- `docs/keeperhub-integration-paths.md:158` claims "the audit chain itself is not duplicated inside the capsule" — **verified true**. The capsule schema's `audit.bundle_ref` is `{"type": "string", "const": "mandate.audit_bundle.v1"}` (a marker, not embedded data). A KeeperHub reviewer relying on this property to bound capsule size can do so safely.
- `FEEDBACK.md:30` claims `mandate_passport_capsule_hash` is "populated once the Passport capsule schema lands on `main` — A-side work tracked at … P1.1". P1.1 (#42) and P2.1 (#44) have both landed. **The schema is on main and the capsule is producible.** The field is no longer blocked on schema work; it's blocked on the (XS) capsule-hash helper. **TRUTHFULNESS GAP**, low-impact (the doc's framing is older than the current state).

---

## Cross-cutting findings

- `PolicyReceipt` carries `audit_event_id`, `request_hash`, `policy_hash`, and `signature.signature_hex` as String fields (`crates/mandate-core/src/receipt.rs:35-55`). **All four IP-1 envelope fields are constructible by struct field reads** — no new field, no new type, no schema bump. This affects IP-1 and IP-5 jointly.
- `mandate-core::hashing::canonical_json` and `request_hash` are public helpers (`hashing.rs:13,23`). The same JCS+SHA256 contract applies to APRP bodies and (target) capsule canonicalisation. Adding a `passport::capsule_hash(&Value)` helper would be a one-line wrapper. This affects IP-1 (`mandate_passport_capsule_hash` field) and IP-5 (the URI-host-side hashing) jointly.
- The `RequiresHuman` decision variant has no representation in `mandate.passport_capsule.v1` (the schema's `decision.result` enum is `{allow, deny}`). `mandate passport run` rejects this case at the boundary with a clear message (`passport.rs:621-636`). This affects IP-5 only — KeeperHub never sees a `requires_human` capsule, so it doesn't propagate into IP-1 envelopes either.
- The README's `243/243` claim is stale by 20 (real: 263/263 post-#44). This is doc layer, but worth picking up in the next submission-copy refresh. **TRUTHFULNESS GAP** repeated from earlier.

---

## Appendix: file-by-file change list

| File | Smallest-change description | IP unlocked | Effort | Risk |
|---|---|---|---|---|
| `crates/mandate-execution/src/keeperhub.rs` | Add `pub struct KeeperHubLiveConfig { webhook_url, bearer_token, timeout }` + `from_env()`; change `pub fn live()` → `pub fn live(cfg)`. Body still `BackendOffline`. | IP-1, IP-4 | XS | XS — two existing tests touch `live()` callsite |
| `crates/mandate-execution/src/keeperhub.rs` | Add `pub struct KeeperHubEnvelope` + `from_receipt(&PolicyReceipt) -> Self` + serde derives. Pure read. | IP-1 | XS | XS |
| `crates/mandate-core/src/passport.rs` | Add `pub fn canonical_capsule_bytes(&Value) -> Result<Vec<u8>>` and `pub fn capsule_hash(&Value) -> Result<String>`. Wraps existing `hashing::canonical_json` + `sha256_hex`. | IP-1, IP-5 | XS | XS |
| `crates/mandate-core/src/lib.rs` + new `mandate-core/src/execution.rs` | Move `GuardedExecutor`, `ExecutionReceipt`, `ExecutionError` from `mandate-execution/src/lib.rs` into `mandate-core::execution`. Re-export from `mandate-execution::lib` for back-compat. | IP-4 | S | S — touches workspace types but the public path is preserved by re-export |
| (new) `crates/mandate-keeperhub-adapter/Cargo.toml`, `src/lib.rs`, `examples/submit_signed_receipt.rs`, `README.md` | Extract `keeperhub.rs` into a publishable crate. Depends on `mandate-core` only. Re-export from `mandate-execution::keeperhub` for back-compat. | IP-4 | M | S |
| `schemas/keeperhub_workflow_submission_v1.json` | New JSON Schema 2020-12 file matching IP-1 envelope. Reference `policy_receipt_v1.json` via `$ref`. | IP-2 | S | XS |
| `scripts/validate_schemas.py` | One-line registration of `keeperhub_workflow_submission_v1`. | IP-2 | XS | XS |
| `crates/mandate-mcp/Cargo.toml` | Add `tokio` (rt-multi-thread, io-util), `serde_json`, `thiserror`. | IP-3 | XS | S — first tokio dependency in this crate |
| `crates/mandate-mcp/src/lib.rs` + `src/main.rs` (new) | Stdio JSON-RPC server with `mandate.audit_lookup(audit_event_id)` and `mandate.verify_capsule(capsule_json)` tool registrations. Handlers re-use existing `audit_bundle::build` + `passport::verify_capsule`. | IP-3 | M | M — first MCP impl |
| `crates/mandate-mcp/tests/stdio_smoke.rs` (new) | One happy-path test driving the stdio server with a JSON-RPC `tools/call`. | IP-3 | S | S |
| `README.md:13` | Update `243/243 green` → `263/263 green`. | (truthfulness) | XS | XS |
| `docs/keeperhub-integration-paths.md:102` | Replace IP-3 sketch regex `^evt-[0-9A-Z]{26}$` with the canonical `^evt-[0-7][0-9A-HJKMNP-TV-Z]{25}$`. | (truthfulness) | XS | XS |
| `docs/keeperhub-integration-paths.md:188` | Update tally `24 real / 0 mock / 1 skipped` → `26 real / 0 mock / 1 skipped` (post-#44). | (truthfulness) | XS | XS |
| `FEEDBACK.md:30` | Re-frame `mandate_passport_capsule_hash` from "blocked on schema landing" to "blocked on the capsule-hash helper" (capsule schema is on main from #42). | (truthfulness) | XS | XS |

---

## Hand-off

**Hand-off file:** `docs/audit/IMPLEMENTATION_AUDIT_KH_PATHS.md` (this document) on branch `docs/kh-implementation-audit`. **Uncommitted.** Working tree only. Daniel reviews before staging.

**Headline findings (one paragraph):** Of the five integration paths, IP-5 is fully producible end-to-end on `main` today; IP-1 needs one ~30-LOC envelope helper; IP-2 needs an ~80-LOC JSON Schema we provide; IP-4 is one ~80-LOC type-move from "trivially extractable" because `mandate-execution` already has zero leaks to policy/storage/server/identity/mcp; IP-3 is the only path with a real surface gap — `crates/mandate-mcp/src/lib.rs` is one comment line. The single highest-leverage change is moving `GuardedExecutor` + `ExecutionReceipt` + `ExecutionError` into `mandate-core` (S effort) — it unlocks IP-4 cleanly and creates the natural home for the IP-1 envelope helper. Four truthfulness gaps flagged: README test count (243→263), IP-3 ULID regex looser than schema, integration-paths tally stale by 2 post-#44, FEEDBACK framing of `mandate_passport_capsule_hash` predates capsule schema landing.
