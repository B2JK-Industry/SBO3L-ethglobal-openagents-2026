# Implementation Status

Live progress tracker. Updated as slices complete.

**Last updated:** 2026-04-28
**Current phase:** Phase 4 — post-merge hardening; security/perf/refactor PRs in flight.
**Current branch:** `feat/nonce-replay-protection`
**Current PR:** [#7](https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026/pull/7) — `feat: enforce protocol.nonce_replay (HTTP 409) on reused APRP nonces`
**CI:** ✅ green on PR #7 (Rust check + JSON Schemas/OpenAPI validators).
**Codex:** ✅ re-reviewed PR #7 — "Didn't find any major issues."

## Merged

- [x] PR #1 — `[WIP] Implement Mandate ETHGlobal Open Agents vertical` (squashed into `main` as `6f137fb`).
- [x] PR #2 — `chore: add Codex (Claude Code) PR review workflow` (`f99cd2e`).

## Open PRs (all CI green, awaiting Daniel's manual review / merge)

| PR | Branch | Title | Status |
|----|--------|-------|--------|
| [#5](https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026/pull/5) | `chore/dedupe-same-origin` | refactor: deduplicate `same_origin` into `mandate-policy::util` | ✅ CI green |
| [#6](https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026/pull/6) | `perf/audit-last-single-query` | perf: collapse `audit_last` into a single query | ✅ CI green |
| [#7](https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026/pull/7) | `feat/nonce-replay-protection` | feat: enforce `protocol.nonce_replay` (HTTP 409) on reused APRP nonces | ✅ CI green, Codex re-reviewed clean |
| [#8](https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026/pull/8) | `tests/null-cmp-and-freeze-all` | tests: null comparison + `emergency.freeze_all` regressions (rebased) | ✅ CI green |
| [#9](https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026/pull/9) | `feat/policy-validation-hardening` | feat: validate policy uniqueness invariants in `Policy::parse_{json,yaml}` | ✅ CI green |

## Pending / stretch

- [ ] Live KeeperHub backend (stub today; one-function-body switch when credentials available).
- [ ] Live ENS testnet resolver (offline fixture today; trait already abstracts the backend).
- [ ] Live Uniswap quote backend (gated behind `MANDATE_UNISWAP_LIVE=1`; static fixture today).
- [ ] Demo video (3:30 cut). Storyboard committed in `demo-scripts/demo-video-script.md`.

## PR #7 — what changed (current branch)

Two commits on top of `main`:

1. `88ed2ff` `feat: enforce protocol.nonce_replay (HTTP 409) on reused APRP nonces`
   - New `seen_nonces: Mutex<HashSet<String>>` on `AppState`.
   - `POST /v1/payment-requests` claims the nonce **before** policy / budget / audit / signing — a replay never mutates state.
   - Returns HTTP `409 Conflict` with deny code `protocol.nonce_replay` on duplicate.
   - Two regression tests: first request succeeds (200), replay rejected (409); concurrent-replay only one wins.
2. `f0e86f1` `docs: clarify replay gate has no audit trail and is rejection-only`
   - Doc-comment fixes for the two items Codex flagged on first pass — now explicit that the in-memory set is reset on restart and that rejected replays are intentionally not chained into the audit log (would let an attacker grow the log with crafted nonces).

## Tests / CI status (PR #7 head)

- `cargo test --workspace --all-targets` — ✅ **71 tests pass** (69 baseline + 2 nonce-replay regression tests).
- `cargo fmt --check` — ✅ (CI).
- `cargo clippy --workspace --all-targets -- -D warnings` — ✅ (CI).
- `python scripts/validate_schemas.py` — ✅ (CI).
- `python scripts/validate_openapi.py` — ✅ (CI).

## Codex review feedback (PR #7)

- Initial review flagged two doc-only issues:
  1. Misleading `seen_nonces` doc comment ("persisted") → fixed in `f0e86f1`.
  2. Claim that replays are audited → fixed in `f0e86f1` (explicit "rejection-only, no audit trail" + rationale).
- Re-review after `f0e86f1`: **"Didn't find any major issues."**

## Next exact task

**Wait for Daniel's manual review on PR #7 before any merge** (per session instruction: security-sensitive change, second pair of eyes required even though Codex is clean).

While waiting, candidate next slices (only after explicit go-ahead — do NOT start in parallel with security review):

1. PR #5 (refactor `same_origin`) — lowest-risk, mechanical dedup.
2. PR #6 (perf `audit_last`) — small SQL change, mirrors the already-merged `audit_list` pattern.
3. PR #8 (regression tests only — no production change).
4. PR #9 (policy uniqueness validation — adds parse-time invariants).

## Blockers

None. Awaiting human review.
