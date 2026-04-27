# Implementation Status

Live progress tracker. Updated as slices complete.

**Last updated:** 2026-04-27
**Current phase:** Phase 3 — vertical slices on `feat/initial-mandate-implementation`
**Branch:** `feat/initial-mandate-implementation`
**PR:** [#1 (draft)](https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026/pull/1)
**CI:** ✅ green on latest commit (Rust check + schemas/OpenAPI validators).

## Done

- [x] Phase 0 — tooling/auth verified (Rust 1.94, gh CLI authed, Node, Python).
- [x] Phase 1 — fresh public repo + `main` initialized + feature branch.
- [x] Phase 2 — planning artifacts seeded under `docs/spec/`; live contracts in `schemas/`, `test-corpus/`, `demo-agents/`, `docs/api/openapi.json`.
- [x] Meta files: `README.md`, `LICENSE`, `AI_USAGE.md`, `IMPLEMENTATION_STATUS.md`, `SUBMISSION_NOTES.md`, `FEEDBACK.md`, `PR_DESCRIPTION.md`.
- [x] Rust workspace + 8 crate skeletons (`mandate-core/policy/storage/identity/execution/mcp/cli/server`).
- [x] CI: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --all-targets`, JSON Schema + OpenAPI validators.
- [x] APRP v1 types with `serde(deny_unknown_fields)`; corpus round-trips.
- [x] JCS canonical hashing + locked golden APRP hash (`c0bd2fab…`).
- [x] JSON Schema validation with embedded schemas + local refs (no network).
- [x] CLI: `mandate aprp validate|hash|run-corpus`, `mandate schema <kind>`, `mandate verify-audit` (stub).
- [x] Ed25519 dev signer (deterministic seed support).
- [x] Policy receipt v1: sign + verify + schema check.
- [x] Decision token v1: sign + verify + schema check.
- [x] Partial `demo-scripts/run-openagents-final.sh` runner.

## In progress

- [ ] Policy engine + budget evaluator (`mandate-policy`).

## Pending

- [ ] SQLite storage + hash-chained audit log + verifier (`mandate-storage`).
- [ ] Payment-request HTTP API (`mandate-server`).
- [ ] Research-agent harness with `legit-x402` and `prompt-injection` scenarios (`demo-agents/research-agent`).
- [ ] ENS identity adapter (`mandate-identity`).
- [ ] KeeperHub guarded-execution adapter (`mandate-execution`).
- [ ] Uniswap guarded-swap adapter (stretch).
- [ ] Replace partial demo runner with full sponsor flow.
- [ ] Codex review requested + feedback addressed.

## Tests / demo status

- `cargo fmt --check` — ✅
- `cargo clippy --workspace --all-targets -- -D warnings` — ✅
- `cargo test --workspace --all-targets` — ✅ 23 unit tests pass.
- `python scripts/validate_schemas.py` — ✅ 6 schemas, 4 fixtures, all expected.
- `python scripts/validate_openapi.py` — ✅ docs/api/openapi.json valid.
- `bash demo-scripts/run-openagents-final.sh` — ✅ partial gates pass; clearly labels what's pending.

## Blockers

None.

## Next exact command

```bash
# Implement mandate-policy: model + expression evaluator + decide() + budget tracker
$EDITOR crates/mandate-policy/src/{model.rs,expr.rs,engine.rs,budget.rs,lib.rs}
cargo test -p mandate-policy
```
