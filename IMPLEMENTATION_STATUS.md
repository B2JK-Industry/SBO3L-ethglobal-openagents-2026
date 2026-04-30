# Implementation Status

Current snapshot for the ETHGlobal Open Agents 2026 submission of **SBO3L**.

**Last updated:** 2026-04-30 (post-cleanup sweep)
**Branch:** `main` (HEAD `29e2135` — post URL slug rename, sponsor narrative refresh, CI security pass, repo URL updates, plus the B1/B2/B3/B7 implementation track)
**Phase:** submission. `main` is implemented, reproducible, and the public proof surface is wired.
**Open implementation PRs:** **0** open at audit time. PRs [#61](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/61) (cryptographic passport verifier — B1) and [#62](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/62) (ScopeBlind IP audit — B2) merged earlier on 2026-04-30 along with the rest of the post-rename cleanup (#63, #64, #67, #69, #71, #72, #73, #74, #75).
**CI on `main`:** ✅ green (`Rust check` + `Validate JSON schemas / OpenAPI`).
**Blockers:** none.

For the **B5 final audit (earlier snapshot)** see [`FINAL_REVIEW_B5.md`](FINAL_REVIEW_B5.md). For the **KeeperHub IP-1..IP-5 implementation audit** see [PR #48](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/48) (DRAFT, doc-only, not yet on `main`; awaiting Daniel review). For the **historical PR-by-PR audit trail** (frozen at `f52596c`, pre PR #11+) see [`FINAL_REVIEW.md`](FINAL_REVIEW.md).

## Verification summary

| Command | Result |
|---|---|
| `cargo fmt --check` | ✅ |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ no warnings |
| `cargo test --workspace --all-targets` | ✅ **377 / 377 pass** (0 fail, 0 ignored) |
| `python3 scripts/validate_schemas.py` | ✅ (7 schemas + 14 corpus fixtures) |
| `python3 scripts/validate_openapi.py` | ✅ (`docs/api/openapi.json` valid) |
| `bash demo-scripts/run-openagents-final.sh` | ✅ all **13 gates** green incl. audit-chain tamper detection and agent no-key proof (~5 seconds end-to-end) |
| `bash demo-scripts/run-production-shaped-mock.sh` | ✅ **Tally: 26 real, 0 mock, 1 skipped** — PSM-A1.9 mock-KMS lifecycle + PSM-A2 four-case Idempotency-Key matrix + PSM-A3 active-policy lifecycle + **PSM-A4 audit checkpoint create/verify with mock anchoring** + PSM-A5 doctor + Passport P2.1 capsule emit/verify all walked end-to-end, plus step 12 emits the `sbo3l-operator-evidence-v1` transcript consumed by the operator console; only the optional `--include-final-demo` flag remains on the SKIPPED list |
| `python3 trust-badge/build.py` | ✅ writes `trust-badge/index.html` (self-contained, no JS, no fetch) |
| `python3 trust-badge/test_build.py` | ✅ **49 stdlib assertions** on the rendered HTML (capsule summary tile + 4 fallback states added in Passport P2.2) |
| `python3 operator-console/build.py` | ✅ writes `operator-console/index.html` (self-contained, no JS, no fetch) — renders the `sbo3l-demo-summary-v1` transcript plus the `sbo3l-operator-evidence-v1` evidence transcript, with one real panel per merged A-side surface |
| `python3 operator-console/test_build.py` | ✅ **118 stdlib assertions** (B2.v2 real-evidence panels — PSM-A2 + PSM-A5 + PSM-A1.9 + PSM-A3 + PSM-A4 — plus the Passport P2.2 capsule panel with both allow + deny tiles and 4 capsule fallback states) |
| `python3 demo-fixtures/test_fixtures.py` | ✅ 4 mock fixtures clean + url-allowlist self-test |

## What is implemented

Full Open Agents vertical:

- Rust workspace (9 crates + research-agent demo binary).
- `sbo3l` CLI: `aprp validate|hash|run-corpus`, `schema`, `verify-audit`, `audit export`, `audit verify-bundle`.
- APRP v1 wire format with `serde(deny_unknown_fields)` end-to-end + JCS canonical request hashing (golden hash `c0bd2fab…` locked in test).
- Strict JSON Schema validation (embedded, local refs, no network).
- Ed25519 dev signer (deterministic seed, public, demo-only; production path via `AppState::with_signers`).
- Policy receipt v1, decision token v1, audit event v1 — all sign + verify + schema-validated.
- Hash-chained audit log with `prev_event_hash` linkage, SQLite-backed storage, structural and strict-hash verifiers.
- Verifiable audit bundle (`sbo3l.audit_bundle.v1`) with both JSONL-chain and DB-backed export paths. The DB-backed exporter pre-flights signature and chain integrity before writing the bundle file.
- Tiny Rego-compatible expression evaluator + `decide()` + canonical policy hash.
- Multi-scope budget tracker (`per_tx`, `daily`, `monthly`, `per_provider`).
- HTTP API: `POST /v1/payment-requests` (full pipeline: schema → request_hash → **persistent SQLite-backed nonce replay gate** → policy → budget → audit → signed receipt) + `GET /v1/health`.
- Persistent APRP nonce-replay store backed by SQLite (`nonce_replay` table, migration V002) accessed via `Storage::nonce_try_claim`. Atomic INSERT-or-fail dedup; survives daemon restart when `Storage::open(path)` is used.
- Real research-agent harness (`legit-x402`, `prompt-injection`) using an in-memory daemon.
- ENS identity adapter (offline fixture resolver + policy_hash verification, trait-backed).
- KeeperHub guarded-execution adapter (`local_mock` + `live` constructor pair; demo uses `local_mock`).
- Uniswap guarded-swap adapter (token allowlist, max notional, max slippage, quote freshness, treasury recipient) + `UniswapExecutor::local_mock()`.
- Sponsor demo scripts: `ens-agent-identity.sh`, `keeperhub-guarded-execution.sh`, `uniswap-guarded-swap.sh`.
- Standalone red-team gate: `demo-scripts/red-team/prompt-injection.sh` (`D-RT-PI-01..03`).
- Reset hook: `demo-scripts/reset.sh`.
- Final demo runner: `bash demo-scripts/run-openagents-final.sh` — single command, **13 gates**, ~5 seconds. Includes: schema gate, locked golden hash, audit-chain structural + strict verify, live `cargo test` of policy/budget/storage/server, real research-agent harness, ENS identity proof, KeeperHub guarded execution, Uniswap guarded swap, red-team prompt-injection gate, audit-chain tamper detection, agent no-key boundary proof, deterministic transcript artifact.
- Production-shaped mock runner: `bash demo-scripts/run-production-shaped-mock.sh` — exercises the operator surface (doctor, mock KMS CLI, active-policy lifecycle, persistent-SQLite allow + deny, audit-bundle export, **audit checkpoint create + verify with mock anchoring**, Passport P2.1 capsule emit/verify pair) end-to-end and emits a `sbo3l-operator-evidence-v1` transcript for the operator console. Tally **26 real / 0 mock / 1 skipped** post-Passport-P2.1; the only SKIPPED item is the optional `--include-final-demo` flag — every A-side backlog row has merged.
- Audit checkpoints + mock anchoring: `sbo3l audit checkpoint {create, verify}` (PSM-A4) backed by SQLite migration V007 (`audit_checkpoints` table). This is **mock anchoring**, NOT real onchain anchoring — the `mock_anchor_ref` is a deterministic local id, never broadcast and never attested by any chain. Every CLI line carries a `mock-anchor:` prefix; `mock_anchor: true` is in every JSON artifact; the verifier refuses any artifact with `mock_anchor: false`. Documented in `docs/cli/audit-checkpoint.md`.
- Active-policy lifecycle: `sbo3l policy {validate, current, activate, diff}` (PSM-A3) backed by SQLite migration V006 (`active_policy` table with partial UNIQUE singleton index). Local lifecycle, not remote governance — `docs/cli/policy.md` documents the scope honestly.
- Static, offline trust-badge proof viewer (`trust-badge/build.py`, stdlib Python) + stdlib regression test (`trust-badge/test_build.py`). No JS, no fetch, works from `file://`.
- **SBO3L Passport capsule schema + verifier + CLI** (`schemas/sbo3l.passport_capsule.v1.json`, `crates/sbo3l-core/src/passport.rs`, `crates/sbo3l-cli/src/passport.rs`, Passport P1.1 + P2.1). `sbo3l passport run` drives the existing `POST /v1/payment-requests` pipeline in-process, reads back the audit event, builds a checkpoint, and self-verifies the capsule before atomic write. `sbo3l passport verify` runs the structural verifier against any capsule. 9 tampered fixtures pin every cross-field invariant.
- **MCP-callable SBO3L gateway** (`crates/sbo3l-mcp/`, Passport P3.1) — stdio JSON-RPC 2.0 server exposing six tools: `sbo3l.validate_aprp`, `sbo3l.decide`, `sbo3l.run_guarded_execution`, `sbo3l.verify_capsule`, `sbo3l.audit_lookup` (the IP-3 sister tool to KeeperHub's proposed `keeperhub.lookup_execution`), and `sbo3l.explain_denial` (machine-readable deny-code lookup). 29 integration tests across in-process dispatch, stdio child-process transport, and path-sandbox escapes. Judge-facing walk-through: [`docs/mcp-integration-guide.md`](docs/mcp-integration-guide.md) (Passport P3.2). Sponsor demo: `bash demo-scripts/sponsors/mcp-passport.sh` (writes `demo-scripts/artifacts/mcp-transcript.json`).
- **KeeperHub `sbo3l_*` envelope helper** (`sbo3l_keeperhub_adapter::build_envelope`, Passport P5.1, IP-1) — composes `sbo3l_request_hash` + `sbo3l_policy_hash` + `sbo3l_receipt_signature` + `sbo3l_audit_event_id` (target: `sbo3l_passport_capsule_hash`) directly from an existing `PolicyReceipt` so a `KeeperHubExecutor::live()` body can drop it onto the workflow webhook submission. **Helper shipped; the live HTTP wiring is still gated on KeeperHub publishing a stable submission/result schema** (see `docs/keeperhub-live-spike.md` §Open questions).
- **Standalone `sbo3l-keeperhub-adapter` workspace crate** (`crates/sbo3l-keeperhub-adapter/`, IP-4) — exposes `KeeperHubExecutor`, `KeeperHubMode`, `build_envelope`, and re-exports `GuardedExecutor` / `ExecutionReceipt` / `ExecutionError` / `Sbo3lEnvelope` from `sbo3l-core`; `sbo3l-execution` re-exports it for back-compat. Crates.io publication remains a target, not a shipped claim.
- **GitHub Pages public proof site** (`.github/workflows/pages.yml` + `site/index.html`, Passport P7.1) — deploys from `main`. Renders the trust-badge + operator-console + a downloadable Passport capsule into a stable static URL. Plain HTML, no JS, no client-side network calls, no external asset; byte-grep-clean against the same safe-host allowlist as `demo-fixtures/test_fixtures.py`.

## Surfaces a judge can verify

- Signed Ed25519 policy receipts (`receipt.signature`).
- Canonical `request_hash` over the JCS-canonicalised APRP body.
- `policy_hash` over the canonicalised active policy.
- `audit_event_id` linking each receipt to a specific position in the audit chain.
- Hash-chained audit log with structural verify (skip-hash) and strict-hash verify modes.
- Audit-chain tamper detection — flip one byte and strict-hash verify rejects.
- Persistent SQLite nonce-replay table — replay returns HTTP 409 `protocol.nonce_replay` before any side effects, persists across daemon restart.
- Verifiable audit bundle (`sbo3l audit export` + `sbo3l audit verify-bundle`), DB-backed export path included.
- Agent no-key proof gate — asserts zero signing references, zero key-material fixtures, no signing-related cargo deps in the agent crate.
- Deterministic transcript artifact written to `demo-scripts/artifacts/latest-demo-summary.json` (consumed by the trust-badge).
- Static, offline trust-badge / proof viewer rendered straight from that transcript.
- **HTTP `Idempotency-Key` safe-retry** (PSM-A2) — persistent SQLite-backed dedup; same-key/same-body → byte-identical cached response, no second audit row; same-key/different-body → 409 `protocol.idempotency_conflict`; different-key + same-nonce → 409 `protocol.nonce_replay` (defense in depth). Migration V004.
- **`sbo3l doctor`** (PSM-A5) — operator readiness summary. Reports per-feature `ok`/`skip`/`warn`/`fail`; refuses to open a missing DB (no write-on-typo); falls through to real `storage_open` errors on permission/IO failures (not "does not exist"). Stable `sbo3l.doctor.v1` JSON envelope.
- **Mock KMS CLI surface + persistence** (PSM-A1.9) — `sbo3l key {init,list,rotate} --mock`; persistent `mock_kms_keys` SQLite table (V005). Every CLI line `mock-kms:`-prefixed; `--mock` mandatory; rotate refuses on mismatched root-seed; current-version lookup propagates real DB errors. **Mock — not production-grade.**
- **Production-shaped mock runner** (`demo-scripts/run-production-shaped-mock.sh`) — exercises the full PSM-A2 four-case matrix, PSM-A5 doctor, PSM-A1.9 init/list/rotate lifecycle, PSM-A3 active-policy lifecycle, PSM-A4 audit-checkpoint create/verify with mock anchoring, Passport P2.1 capsule emit/verify, and the operator-evidence transcript emission end-to-end against real binaries; `Tally: 26 real, 0 mock, 1 skipped`.
- **Static, offline operator console** (`operator-console/build.py`) — sister surface to the trust-badge: vertical timeline + multi-panel grid + **five real-evidence panels** (PSM-A2 four-case Idempotency-Key matrix, PSM-A5 `sbo3l doctor --json` grouped report, PSM-A1.9 mock KMS keyring table, PSM-A3 active-policy lifecycle, PSM-A4 audit checkpoint with explicit `mock anchoring, NOT onchain` pill) rendered straight from the production-shaped runner's `sbo3l-operator-evidence-v1` transcript. Zero pending pills, zero blocked pills. When the evidence transcript is missing/malformed/wrong-schema each panel renders an honest "evidence not gathered" placeholder — never a fake-OK.
- **Production-shaped mock fixtures** (`demo-fixtures/mock-*.json`) — ENS multi-agent registry, KeeperHub workflow envelopes (success/conflict/refused/lookup), Uniswap quote catalogue (happy/multi-violation rug/recipient-allowlist), mock-KMS public keyring metadata. Plus stdlib validator (`test_fixtures.py`) with `urlparse`-based safe-host allowlist + URL-bypass self-test.
- **Per-fixture production-transition guides** (`demo-fixtures/mock-*.md`) and a single **`docs/production-transition-checklist.md`** — every surface (ENS / KeeperHub / Uniswap / Signer-KMS-HSM) has env vars / endpoints / credentials / code-change steps / verification / truthfulness invariants spelled out.

## Live integrations (operator-supplied env vars; demo defaults to mock for CI determinism)

- **Live KeeperHub** — `KeeperHubExecutor::live()` (in `crates/sbo3l-keeperhub-adapter/src/lib.rs`) POSTs the IP-1 envelope to a real workflow webhook and captures the returned `executionId` into the `ExecutionReceipt`. Activated when both `SBO3L_KEEPERHUB_WEBHOOK_URL` and `SBO3L_KEEPERHUB_TOKEN` (a `wfb_*` token, NOT `kh_*`) are set in the daemon's environment. Verified end-to-end against a real KeeperHub workflow during the submission window.
- **Live ENS resolver** — `LiveEnsResolver` (in `crates/sbo3l-identity/src/ens_live.rs`) reads the five `sbo3l:*` text records from a real ENS Public Resolver via JSON-RPC. Activated by `SBO3L_ENS_RPC_URL`. Smoke example: `cargo run -p sbo3l-identity --example ens_live_smoke` against `sbo3lagent.eth` (mainnet, owned by the team) returns the live records.
- **Live Uniswap quote** — `UniswapExecutor::live()` (in `crates/sbo3l-execution/src/uniswap_live.rs`) issues a real `quoteExactInputSingle` call against the Sepolia QuoterV2 contract `0xEd1f6473345F45b75F8179591dd5bA1888cf2FB3`. Activated by `SBO3L_UNISWAP_RPC_URL` (must be a Sepolia endpoint). Smoke example: `cargo run -p sbo3l-execution --example uniswap_live_smoke`.

The demo runners (`run-openagents-final.sh`, `run-production-shaped-mock.sh`) deliberately keep `local_mock()` defaults so CI stays offline and deterministic. Live integrations are exercised out-of-band via the smoke examples and the env-var-gated paths above.

## Pending / stretch (not blocking submission)

- Production KMS / HSM signer (`SBO3L_SIGNER_BACKEND` selector + per-role `SBO3L_*_SIGNER_KEY_ID` env vars). The dev `DevSigner` and the persistent mock `MockKmsSigner` are both `⚠ DEV ONLY ⚠`; production injects real signers via `AppState::with_signers`.
- Recorded demo video (3:30 cut). Script committed in `demo-scripts/demo-video-script.md`.
- Pruned / Merkle-proof variants of the audit bundle, and optional embedded original APRP. Tracked in `docs/cli/audit-bundle.md`.
- Soft-cap warning emission in receipts (`Budget.soft_cap_usd` parsed but not enforced).
- B5 — final submission package wiring (transcripts/, recorded video URL, etc).

## Blockers

**None.**
