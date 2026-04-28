# Implementation Status

Post-merge snapshot for the ETHGlobal Open Agents 2026 submission.

**Last updated:** 2026-04-28
**Phase:** 5 — submission readiness; all hardening PRs merged.
**`main` HEAD:** `f52596c433861c72c2f22ffe183674524d45e14d` (pre-docs-PR; this status file lives on `docs/final-submission-readiness`).
**Open PRs:** 0 implementation PRs. One **docs-only** PR open (`docs: finalize ETHGlobal submission readiness`) carrying this file, `FINAL_REVIEW.md`, and the doc cleanups identified by the Phase-5 review.
**CI on `main`:** ✅ green on the latest commit (`Rust check` + `Validate JSON schemas / OpenAPI`).

## Merged PRs

| PR | Merge SHA | Title |
|----|-----------|-------|
| [#1](https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026/pull/1) | `6f137fb` | `[WIP] Implement Mandate ETHGlobal Open Agents vertical` (full vertical) |
| [#2](https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026/pull/2) | `f99cd2e` | `chore: add Codex (Claude Code) PR review workflow` |
| [#7](https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026/pull/7) | `2c3eb70` | `feat: enforce protocol.nonce_replay (HTTP 409) on reused APRP nonces` |
| [#9](https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026/pull/9) | `8e24154` | `feat: validate policy uniqueness invariants in Policy::parse_{json,yaml}` |
| [#8](https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026/pull/8) | `931fb28` | `tests: null comparison + emergency.freeze_all regressions` |
| [#6](https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026/pull/6) | `30fb407` | `perf: collapse audit_last into a single query` |
| [#5](https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026/pull/5) | `f52596c` | `refactor: deduplicate same_origin into mandate-policy::util` |

## What is implemented

Full Open Agents vertical:

- Rust workspace (8 crates + research-agent demo bin).
- `mandate` CLI: `aprp validate|hash|run-corpus`, `schema`, `verify-audit`.
- APRP v1 wire format with `serde(deny_unknown_fields)` end-to-end + JCS canonical request hashing (golden hash `c0bd2fab…` locked in test).
- Strict JSON Schema validation (embedded, local refs, no network).
- Ed25519 dev signer (deterministic seed; production path via `AppState::with_signers`).
- Policy receipt v1, decision token v1, audit event v1 — all sign + verify + schema-validated.
- Hash-chained audit log with `prev_event_hash` linkage and SQLite-backed storage.
- Tiny Rego-compatible expression evaluator + `decide()` + canonical policy hash.
- Multi-scope budget tracker (`per_tx`, `daily`, `monthly`, `per_provider`) — accumulating where it should, non-accumulating where it shouldn't.
- HTTP API: `POST /v1/payment-requests` (full pipeline: schema → request_hash → **nonce replay gate** → policy → budget → audit → signed receipt) + `GET /v1/health`.
- Real research-agent harness (`legit-x402`, `prompt-injection`) using an in-memory daemon.
- ENS identity adapter (offline fixture resolver + policy_hash verification).
- KeeperHub guarded-execution adapter (`local_mock` + `live` constructor pair; demo uses `local_mock`).
- Uniswap guarded-swap adapter (token allowlist, max notional, max slippage, quote freshness, treasury recipient) + `UniswapExecutor::local_mock()`.
- Sponsor demo scripts: `ens-agent-identity.sh`, `keeperhub-guarded-execution.sh`, `uniswap-guarded-swap.sh`.
- Standalone red-team gate: `demo-scripts/red-team/prompt-injection.sh` (`D-RT-PI-01..03`).
- Reset hook: `demo-scripts/reset.sh`.
- Final demo runner: `bash demo-scripts/run-openagents-final.sh` — single command, 11 steps, ~5 seconds, includes audit-chain tamper detection.

## Hardening landed during this phase

- **PR #7** — APRP nonce replay protection (HTTP 409 `protocol.nonce_replay`). The replay gate fires before `request_hash` / policy / budget / audit / signing, so a duplicate nonce produces no audit/receipt side effects. Three regression tests cover (a) replay rejected, (b) distinct nonces independently processed, (c) replay with same nonce but mutated body still rejected. In-memory dedup set; resets on daemon restart (documented in `SUBMISSION_NOTES.md` "Known limitations").
- **PR #9** — `Policy::parse_{json,yaml}` reject duplicate `agents[].agent_id`, `rules[].id`, `providers[].id`, `(recipients[].address.lc, chain)`, and `(budgets[].agent_id, scope, scope_key)`. Both parse paths route through the same `validate()` step.
- **PR #8** — Regression tests for `null` comparison semantics in `expr.rs` and the `emergency.freeze_all` global kill-switch in `engine.rs`. Pure additive: `+57` lines across two `#[test]` modules.
- **PR #6** — `audit_last` collapsed from a `SELECT seq` + `audit_get` two-roundtrip into a single `SELECT * ORDER BY seq DESC LIMIT 1`. Tightens error handling: previously `.ok()` swallowed every SQLite error into `Ok(None)`; now only `QueryReturnedNoRows` becomes `None`.
- **PR #5** — `same_origin` deduplicated from `engine.rs` and `budget.rs` into `mandate-policy::util` (`pub(crate)`). Behaviour-preserving; substring-trap test pinned (`example.com.attacker.com` correctly rejected).

## Tests / CI status

- `cargo fmt --check` — ✅
- `cargo clippy --workspace --all-targets -- -D warnings` — ✅ (no warnings)
- `cargo test --workspace --all-targets` — ✅ **90 / 90 pass** (0 fail, 0 ignored)
- `python3 scripts/validate_schemas.py` — ✅ (6 schemas + 4 corpus fixtures)
- `python3 scripts/validate_openapi.py` — ✅ (`docs/api/openapi.json` valid)
- `bash demo-scripts/run-openagents-final.sh` — ✅ all 11 steps green incl. audit-chain tamper detection (~5 seconds end-to-end)

## Pending / stretch (not blocking submission)

- Live KeeperHub backend (one-constructor switch via `KeeperHubExecutor::live()`; demo uses `local_mock()`).
- Live ENS testnet resolver (offline fixture today; trait already abstracts the backend).
- Live Uniswap quote backend (`UniswapExecutor::live()` is intentionally stubbed; demo uses `local_mock()`).
- Demo video (3:30 cut). Storyboard committed in `demo-scripts/demo-video-script.md`.

## Blockers

**None.** See `FINAL_REVIEW.md` for the full submission-readiness audit.
