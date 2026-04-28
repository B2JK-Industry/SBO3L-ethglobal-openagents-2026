# Implementation Status

Current snapshot for the ETHGlobal Open Agents 2026 submission of **Mandate**.

**Last updated:** 2026-04-28
**Branch:** `main`
**Phase:** submission. `main` is implemented, reproducible, and ready to submit.
**Open implementation PRs:** none.
**CI on `main`:** ✅ green (`Rust check` + `Validate JSON schemas / OpenAPI` + trust-badge regression test).
**Blockers:** none.

For the historical PR-by-PR audit trail, see [`FINAL_REVIEW.md`](FINAL_REVIEW.md).

## Verification summary

| Command | Result |
|---|---|
| `cargo fmt --check` | ✅ |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ no warnings |
| `cargo test --workspace --all-targets` | ✅ **121 / 121 pass** (0 fail, 0 ignored) |
| `python3 scripts/validate_schemas.py` | ✅ (6 schemas + 4 corpus fixtures) |
| `python3 scripts/validate_openapi.py` | ✅ (`docs/api/openapi.json` valid) |
| `bash demo-scripts/run-openagents-final.sh` | ✅ all **13 gates** green incl. audit-chain tamper detection and agent no-key proof (~5 seconds end-to-end) |
| `python3 trust-badge/build.py` | ✅ writes `trust-badge/index.html` (self-contained, no JS, no fetch) |
| `python3 trust-badge/test_build.py` | ✅ 31 stdlib assertions on the rendered HTML |

## What is implemented

Full Open Agents vertical:

- Rust workspace (8 crates + research-agent demo binary).
- `mandate` CLI: `aprp validate|hash|run-corpus`, `schema`, `verify-audit`, `audit export`, `audit verify-bundle`.
- APRP v1 wire format with `serde(deny_unknown_fields)` end-to-end + JCS canonical request hashing (golden hash `c0bd2fab…` locked in test).
- Strict JSON Schema validation (embedded, local refs, no network).
- Ed25519 dev signer (deterministic seed, public, demo-only; production path via `AppState::with_signers`).
- Policy receipt v1, decision token v1, audit event v1 — all sign + verify + schema-validated.
- Hash-chained audit log with `prev_event_hash` linkage, SQLite-backed storage, structural and strict-hash verifiers.
- Verifiable audit bundle (`mandate.audit_bundle.v1`) with both JSONL-chain and DB-backed export paths. The DB-backed exporter pre-flights signature and chain integrity before writing the bundle file.
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
- Static, offline trust-badge proof viewer (`trust-badge/build.py`, stdlib Python) + stdlib regression test (`trust-badge/test_build.py`). No JS, no fetch, works from `file://`.

## Surfaces a judge can verify

- Signed Ed25519 policy receipts (`receipt.signature`).
- Canonical `request_hash` over the JCS-canonicalised APRP body.
- `policy_hash` over the canonicalised active policy.
- `audit_event_id` linking each receipt to a specific position in the audit chain.
- Hash-chained audit log with structural verify (skip-hash) and strict-hash verify modes.
- Audit-chain tamper detection — flip one byte and strict-hash verify rejects.
- Persistent SQLite nonce-replay table — replay returns HTTP 409 `protocol.nonce_replay` before any side effects, persists across daemon restart.
- Verifiable audit bundle (`mandate audit export` + `mandate audit verify-bundle`), DB-backed export path included.
- Agent no-key proof gate — asserts zero signing references, zero key-material fixtures, no signing-related cargo deps in the agent crate.
- Deterministic transcript artifact written to `demo-scripts/artifacts/latest-demo-summary.json` (consumed by the trust-badge).
- Static, offline trust-badge / proof viewer rendered straight from that transcript.

## Pending / stretch (not blocking submission)

- Live KeeperHub backend (one-constructor switch via `KeeperHubExecutor::live()`; demo uses `local_mock()`).
- Live ENS testnet resolver (offline fixture today; trait already abstracts the backend).
- Live Uniswap quote backend (`UniswapExecutor::live()` is intentionally stubbed; demo uses `local_mock()`).
- Recorded demo video (3:30 cut). Script committed in `demo-scripts/demo-video-script.md`.
- Pruned / Merkle-proof variants of the audit bundle, and optional embedded original APRP. Tracked in `docs/cli/audit-bundle.md`.
- Soft-cap warning emission in receipts (`Budget.soft_cap_usd` parsed but not enforced).

## Blockers

**None.**
