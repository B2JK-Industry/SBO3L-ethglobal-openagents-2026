# Implementation Status

Live progress tracker. Updated as slices complete.

**Last updated:** 2026-04-27
**Current phase:** Phase 3 — Open Agents vertical green; awaiting Codex review.
**Branch:** `feat/initial-mandate-implementation`
**PR:** [#1 (draft)](https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026/pull/1)
**CI:** ✅ green on latest commit (Rust check + schemas/OpenAPI validators).

## Done

- [x] Phase 0 — tooling/auth verified (Rust 1.94, gh CLI authed, Node, Python).
- [x] Phase 1 — fresh public repo + `main` initialized + feature branch.
- [x] Phase 2 — planning artifacts seeded under `docs/spec/`; live contracts in `schemas/`, `test-corpus/`, `demo-agents/`, `docs/api/openapi.json`.
- [x] Meta files: `README.md`, `LICENSE`, `AI_USAGE.md`, `IMPLEMENTATION_STATUS.md`, `SUBMISSION_NOTES.md`, `FEEDBACK.md`, `PR_DESCRIPTION.md`.
- [x] Rust workspace + 8 crates + research-agent demo bin.
- [x] CI: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --all-targets`, JSON Schema + OpenAPI validators.
- [x] APRP v1 types + JCS canonical hashing + locked golden hash (`c0bd2fab…`).
- [x] JSON Schema validation (embedded, local refs, no network).
- [x] CLI: `mandate aprp validate|hash|run-corpus`, `mandate schema`, `mandate verify-audit`.
- [x] Ed25519 dev signer (deterministic seed support).
- [x] Policy receipt v1 sign + verify + schema check.
- [x] Decision token v1 sign + verify + schema check.
- [x] Audit event v1 sign + verify + chain helper + schema check.
- [x] Policy YAML/JSON model + tiny Rego-compatible expression evaluator + decide() + canonical policy hash.
- [x] Budget tracker (per_tx, daily, monthly, per_provider).
- [x] SQLite storage with migrations + audit log + chain verifier.
- [x] HTTP API: `POST /v1/payment-requests`, `GET /v1/health`. Full pipeline: schema → request_hash → policy → budget → audit → signed receipt.
- [x] Real research-agent harness (`legit-x402`, `prompt-injection`) using in-memory daemon.
- [x] ENS identity adapter (offline fixture resolver + policy_hash verification).
- [x] KeeperHub guarded-execution adapter (live mode stub + faithful local mock).
- [x] Uniswap guarded-swap adapter (`mandate-execution::uniswap`): swap-policy guard (token allowlist, max notional, max slippage, quote freshness, treasury recipient) + `UniswapExecutor::local_mock()` mirror of the KeeperHub pattern.
- [x] Sponsor demo scripts: `demo-scripts/sponsors/ens-agent-identity.sh`, `keeperhub-guarded-execution.sh`, `uniswap-guarded-swap.sh`.
- [x] Standalone red-team gate: `demo-scripts/red-team/prompt-injection.sh` (D-RT-PI-01..03).
- [x] Reset hook: `demo-scripts/reset.sh`.
- [x] Full demo runner: `bash demo-scripts/run-openagents-final.sh` (end-to-end Open Agents vertical green; includes audit-chain tamper detection).

## In progress

- [ ] Codex review requested + feedback addressed.

## Pending / stretch

- [ ] Live KeeperHub backend (stub today; one-function-body switch when credentials available).
- [ ] Live ENS testnet resolver (offline fixture today; trait already abstracts the backend).
- [ ] Live Uniswap quote backend (gated behind `MANDATE_UNISWAP_LIVE=1`; static fixture today).
- [ ] Demo video (3:30 cut). Storyboard committed in `demo-scripts/demo-video-script.md`.

## Tests / demo status

- `cargo fmt --check` — ✅
- `cargo clippy --workspace --all-targets -- -D warnings` — ✅
- `cargo test --workspace --all-targets` — ✅ 62 unit/integration tests pass (52 + 10 new uniswap tests).
- `python scripts/validate_schemas.py` — ✅ 6 schemas, 4 fixtures.
- `python scripts/validate_openapi.py` — ✅ docs/api/openapi.json valid.
- `bash demo-scripts/run-openagents-final.sh` — ✅ all gates pass (steps 1–11 including tamper detection).
- `bash demo-scripts/sponsors/ens-agent-identity.sh` — ✅
- `bash demo-scripts/sponsors/keeperhub-guarded-execution.sh` — ✅
- `bash demo-scripts/sponsors/uniswap-guarded-swap.sh` — ✅ allow + deny.
- `bash demo-scripts/red-team/prompt-injection.sh` — ✅ D-RT-PI-01..03.
- `./demo-agents/research-agent/run --scenario legit-x402` — ✅ auto_approved + signed receipt.
- `./demo-agents/research-agent/run --scenario prompt-injection` — ✅ rejected + deny_code.
- `./demo-agents/research-agent/run --uniswap-quote demo-fixtures/uniswap/quote-USDC-ETH.json --policy demo-fixtures/uniswap/mandate-policy.json --execute-uniswap` — ✅ allow + uni-<ULID>.
- `./demo-agents/research-agent/run --uniswap-quote demo-fixtures/uniswap/quote-USDC-RUG.json --policy demo-fixtures/uniswap/mandate-policy.json --execute-uniswap` — ✅ deny + sponsor refused.

## Blockers

None.

## Next exact command

```bash
gh pr comment 1 --repo B2JK-Industry/mandate-ethglobal-openagents-2026 \
  --body "@codex please review this PR for correctness, security, tests, demo reliability, and ETHGlobal submission readiness."
```

Then watch:

```bash
gh pr view 1 --comments
gh pr checks 1
```
