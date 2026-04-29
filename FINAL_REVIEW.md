# Final Review — SBO3L, ETHGlobal Open Agents 2026

> **HISTORICAL DOCUMENT.** This review captured the submission-readiness audit at commit `f52596c` (post PR #5–#9, pre PR #11+). The current state of `main` is tracked in [`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md) — **121 / 121 tests** and **13 demo gates** (the original 11 plus the agent no-key boundary proof and the deterministic transcript artifact). Persistent SQLite-backed APRP nonce-replay protection, the verifiable audit bundle (with DB-backed export), and the static trust-badge proof viewer all landed in subsequent PRs (#11, #14, #15, #16, #17, #18, #19) and are reflected in [`SUBMISSION_FORM_DRAFT.md`](SUBMISSION_FORM_DRAFT.md). The findings table below is preserved for the audit trail.

**Reviewed commit:** `f52596c433861c72c2f22ffe183674524d45e14d`
**Date:** 2026-04-28
**Scope:** Read-only audit of `main` after merging PRs #5, #6, #7, #8, #9.
**Review verdict before this docs cleanup:** `READY_AFTER_FIXES` — no code/security/demo blockers; only doc staleness issues.
**After the fixes in PR #10:** **`READY_FOR_SUBMISSION`** — all medium and low findings (M1–M4, L1, L2) are resolved by PR #10, as marked in §6 below.

---

## 1. Repository state

| Check | Result |
|---|---|
| Local tree clean | ✅ `git status -sb` empty |
| Open PRs | 0 |
| Latest CI on `main` | ✅ success (`refactor: deduplicate same_origin into sbo3l-policy::util (#5)`, run 25040118315, 29s) |
| All five hardening PRs merged in order | ✅ `f52596c (#5) → 30fb407 (#6) → 931fb28 (#8) → 8e24154 (#9) → 2c3eb70 (#7) → 6f137fb (#1)` |
| Branch protection on `main` | ⚠️ classic branch-protection API returns 404. May be configured via GitHub Rulesets (newer mechanism, separate API) — repo-admin note, not a submission blocker. See finding M5. |

---

## 2. Verification suite

All commands run from a clean checkout against `f52596c`.

| Command | Result | Notes |
|---|---|---|
| `cargo fmt --check` | ✅ | 0.28s |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ | 1.99s — no warnings |
| `cargo test --workspace --all-targets` | ✅ **90 / 90 pass, 0 fail, 0 ignored** | per-test-line count = 90; sum of `test result` lines = 90 |
| `python3 scripts/validate_schemas.py` | ✅ | 6 schemas + 4 corpus fixtures (golden + adversarial + policy) |
| `python3 scripts/validate_openapi.py` | ✅ | `docs/api/openapi.json` valid |
| `bash demo-scripts/run-openagents-final.sh` | ✅ all 11 steps green | 4.42s end-to-end. Three lines match `error|fail|warn` — all are intentional rug-token deny output (`FAIL output_token_allowlisted` etc., the expected Uniswap deny path). No flakes observed. |

Test breakdown (per-crate `test result` summaries):
- `sbo3l-core` — 27 (APRP, hashing, signer, receipt, decision_token, audit, schema)
- `sbo3l-policy` — 21 + 13 + 5 across `model` / `engine` / `expr` / `budget` / `util`
- `sbo3l-storage` — 4 (audit_append + verify, audit_last × 2, migrations idempotent)
- `sbo3l-server` — 6 (legit / prompt-injection / adversarial / replay × 3)
- `sbo3l-identity` — 3
- `sbo3l-execution` — 3 (KeeperHub) + Uniswap suite

---

## 3. Security & correctness review

Methodology: an Explore subagent gathered evidence with file paths and line ranges; I spot-checked the most security-relevant claims (nonce gate placement, freeze_all rule, agent-side keys, validate() body) directly. All claims hold.

| Area | Verdict | Key evidence |
|---|---|---|
| 1. APRP schema strictness | **PASS** | `schemas/aprp_v1.json:6` `additionalProperties: false`; `crates/sbo3l-core/src/aprp.rs:7` `#[serde(deny_unknown_fields)]` (and on every nested struct + enum); test `unknown_field_is_rejected_by_serde` covers the adversarial fixture. |
| 2. Canonical hashing | **PASS** | `crates/sbo3l-core/src/hashing.rs:14` JCS via `serde_json_canonicalizer`; `golden_aprp_hash_is_locked` test pins `c0bd2fab…`. |
| 3. Nonce replay protection | **PASS** | `crates/sbo3l-server/src/lib.rs:181-194` gate before `request_hash`/policy/budget/audit/sign. Replay key is `aprp.nonce` only. HTTP 409 + `protocol.nonce_replay`. Three regression tests: replay-rejected, distinct-nonces-OK, mutated-body-same-nonce-still-rejected (no `receipt`/`audit_event_id` in response). |
| 4. Policy validation | **PASS** | `crates/sbo3l-policy/src/model.rs:214-269` validates all five uniqueness invariants (agent_id, rule.id, provider.id, recipient `(addr.lc, chain)`, budget `(agent_id, scope, scope_key)`). Both `parse_json` and `parse_yaml` route through `validate()`. |
| 5. Policy decisions | **PASS** | Allow returns `matched_rule_id`; deny returns deterministic `deny_code`. `deny-emergency-freeze` rule in `test-corpus/policy/reference_low_risk.json:55-59` triggers on `input.emergency.freeze_all == true`. `null` semantics: `==` identity-true/false, `<` errors. `same_origin` rejects `example.com.attacker.com` substring trap. |
| 6. Budget semantics | **PASS** | `per_tx` does not accumulate (`commit()` skips `BudgetScope::PerTx`); daily/monthly/per_provider do. `commit()` runs only after Allow. Replays are rejected before reaching budget check (nonce gate is upstream). |
| 7. Audit chain | **PASS** | `audit_append` chains via `prev_event_hash` → `event_hash`. `audit_verify` walks chain + verifies signatures. Post-PR-#6: `audit_last` returns `Ok(None)` only on `QueryReturnedNoRows`; other SQLite errors propagate. |
| 8. Receipts & signatures | **PASS** | Ed25519 over canonical JSON for receipts, decision tokens, audit events. Tampering tests assert verification failure. **No private keys committed.** Dev signing seeds are deterministic constants in `sbo3l-server/src/lib.rs:51-65` explicitly labelled `⚠ DEV ONLY ⚠` with a `with_signers()` injection path for production. |
| 9. Agent boundary | **PASS** | `demo-agents/research-agent/src/main.rs` posts APRP JSON to `/v1/payment-requests`; deny comes from the policy engine response, not an internal agent check. `grep` for signing keys in `demo-agents/` returns nothing. The prompt-injection scenario sends a real APRP from `test-corpus/aprp/deny_prompt_injection_request.json`. |

**Cross-cutting:**
- Fail-closed design: nonce gate / agent_gate / freeze rule all run before any state-mutating step.
- Determinism: JCS hashing, set-based uniqueness, dev seeds make rebuilds reproducible.
- No `BEGIN PRIVATE KEY`, `api_key`, `secret_key` in committed source.

---

## 4. Demo truthfulness

| Question | Answer |
|---|---|
| What is real? | APRP wire format, JCS hashing, schema validation, policy engine, budget tracker, hash-chained audit, signed receipts/decision tokens/audit events, Ed25519 signing/verify, full HTTP pipeline, agent harness sending real cross-boundary requests. |
| What is mocked? | (1) **ENS testnet resolver** — demo uses a local fixture; the trait abstraction is real. (2) **KeeperHub backend** — the demo always constructs `KeeperHubExecutor::local_mock()` (verified at `demo-agents/research-agent/src/main.rs:310`). A `KeeperHubExecutor::live()` constructor exists, but no env-var or runtime feature flag switches between them in this hackathon build. (3) **Uniswap backend** — the demo always constructs `UniswapExecutor::local_mock()` (`demo-agents/research-agent/src/main.rs:331`). `UniswapExecutor::live()` is intentionally stubbed and returns `ExecutionError::BackendOffline`. (4) **Signing seeds** — deterministic dev seeds in `sbo3l-server/src/lib.rs`, gated for production via `AppState::with_signers()`. **There is no `MANDATE_*_LIVE` env-var feature flag anywhere in the build.** |
| Are mocks clearly labelled? | ✅ Demo output prints `keeperhub.sponsor: keeperhub` + `keeperhub mock` mid-flow; Uniswap output uses obviously-fake `qt-01HZFAKEDEMOQ001` quote_id. `SUBMISSION_NOTES.md` has an explicit "What is live vs mocked" section. |
| Does the demo ever pretend a mock is live? | ❌ no. (Minor: the final summary line `KeeperHub executed` is sligthly soft on the `mock` qualifier compared to the mid-flow output, but earlier lines establish the mock context — see L1.) |
| Does the demo prove allow + deny paths? | ✅ legit-x402 → allow + signed receipt; prompt-injection + Uniswap rug-token → deny + `keeperhub.refused`. |
| Does the demo prove audit verification? | ✅ step 11 "Audit chain tamper detection": 3 events linked, structural verify passes, strict-hash verify rejects a tampered event. |
| Does the demo prove the agent has no key? | ✅ implicit (agent crate has no signing keys; verifiable via grep). The demo doesn't make this explicit on screen — could be sharper for the video, but the architectural claim is true. |
| Outdated language ("partial", "TODO", wrong test counts) in demo output | ❌ none in demo runtime output. **Stale "69 tests" appears in `SUBMISSION_NOTES.md:28`.** See M3. |
| Uniswap status | Included in final demo (step 9 of `run-openagents-final.sh`); also documented as "stretch" in `SUBMISSION_NOTES.md:43`. Consistent. |

---

## 5. ETHGlobal submission readiness

| Check | Result |
|---|---|
| README tells judges how to run the demo | ⚠ has the command but is preceded by stale "Pre-implementation. Repo bootstrap in progress." (M1) and "How to run the demo (when ready)" (M2). |
| Demo command works from a clean clone | ✅ (verified above, 4.42s) |
| AI usage transparent | ✅ `AI_USAGE.md` is detailed and honest |
| Pre-hackathon specs clearly distinguished | ✅ README, AI_USAGE, SUBMISSION_NOTES all attribute `docs/spec/` to the pre-hackathon `agent-vault-os` repo |
| Partner integrations described honestly | ✅ `SUBMISSION_NOTES.md` "What is live vs mocked" + `FEEDBACK.md` per-partner |
| Known limitations explicit | ✅ `SUBMISSION_NOTES.md` "Known limitations" section |
| Video script ≤ 4 minutes | ✅ 3:50 hard stop, 3:30 target |
| No language suggesting pre-hackathon build | ✅ AI_USAGE explicit + the "pre-hackathon planning artifacts" framing is consistent |
| Project name consistently `SBO3L` | ✅ verified across all user-facing docs |
| Old names (`Agent Vault OS`, `Vault 402`) absent from user-facing submission text | ✅ only appear in spec docs and as the *historical* name `agent-vault-os` of the planning repo (intentional, transparent attribution) |

---

## 6. Findings

### Blockers
**None.**

### High severity
**None.**

### Medium severity (doc-only; misleading to judges)

- **M1.** ✅ **Resolved by PR #10.** `README.md:13` formerly said `**Status:** Pre-implementation. Repo bootstrap in progress.` — replaced with the accurate post-merge status that points to `IMPLEMENTATION_STATUS.md` and `FINAL_REVIEW.md`.
- **M2.** ✅ **Resolved by PR #10.** `README.md:22` formerly read `## How to run the demo (when ready)` — `(when ready)` dropped, fresh-clone instructions made copy-pasteable.
- **M3.** ✅ **Resolved by PR #10.** `SUBMISSION_NOTES.md:28` test count updated `69 → 90`.
- **M4.** ✅ **Resolved by PR #10.** `IMPLEMENTATION_STATUS.md` rewritten as a post-merge snapshot with all seven implementation PRs (`#1`, `#2`, `#5`, `#6`, `#7`, `#8`, `#9`) listed as merged; no implementation PRs open; cites 90/90 tests.
- **M5.** *(Repo-admin note, not a submission issue.)* The classic branch-protection API for `main` returns HTTP 404. This may simply mean protection is configured via the newer **GitHub Rulesets** mechanism (separate API at `repos/{owner}/{repo}/rulesets`) rather than classic branch protection — the two are often confused. **Not a code or submission blocker.** If neither is configured, enabling either before broader open-source adoption is a fine post-hackathon repo-admin task. *(PR #10's `BLOCKED` mergeStateStatus despite green CI suggests Rulesets are in fact active.)*

### Low severity (polish)

- **L1.** ✅ **Resolved by PR #10.** Demo final summary now reads `KeeperHub mock executed (kh-<ULID>)` and `Bounded USDC -> ETH swap allowed (uni-<ULID> via Uniswap mock executor); rug-token swap denied.` — a viewer skimming only the summary now sees the mock context.
- **L2.** ✅ **Resolved by PR #10.** `PR_DESCRIPTION.md` deleted (was the WIP body of merged PR #1; GitHub preserves the merged PR's history).

---

## 7. Fixes landed in PR #10

All medium and low findings from §6 are resolved in the docs-only PR #10 that carries this report. See the audit trail in §8.

| ID | File touched | Fix |
|---|---|---|
| M1 | `README.md` | Replaced "Pre-implementation. Repo bootstrap in progress." with the accurate post-merge status. |
| M2 | `README.md` | Dropped "(when ready)" from the demo heading; added copy-pasteable `git clone` instructions. |
| M3 | `SUBMISSION_NOTES.md` | Test count bumped `69 → 90`. Also rewrote the "no idempotency / dedup" limitation to reflect the post-#7 reality. |
| M4 | `IMPLEMENTATION_STATUS.md` | Rewritten as a post-merge snapshot that stays true after PR #10 merges. |
| L1 | `demo-scripts/run-openagents-final.sh` | Final summary lines now carry `mock executed` / `via … mock executor` qualifiers. |
| L2 | `PR_DESCRIPTION.md` | Deleted. |
| (also) | `FEEDBACK.md`, `demo-scripts/sponsors/uniswap-guarded-swap.sh`, `demo-agents/research-agent/README.md` | Codex-flagged stale env-var claims removed; ETHPrague→ETHGlobal Open Agents and Vault→SBO3L language updated. |

M5 (branch-protection / Rulesets) is unaffected by this PR and is a post-hackathon repo-admin item, not a submission blocker.

---

## 8. Final verdict

# READY_FOR_SUBMISSION after PR #10

The codebase audited at `f52596c` was solid: build, tests, demo all green; all nine security/correctness areas pass with code-level evidence; mocks are honestly labelled. The only outstanding issues at that point were **stale documentation strings** that would have misled a judge skimming README/SUBMISSION_NOTES — captured as M1–M4 + L1 + L2.

**All of those findings are resolved in PR #10**, which carries this report alongside the doc cleanup. Once PR #10 lands on `main`, the repository is `READY_FOR_SUBMISSION`. The only remaining non-blocking item is M5 (a repo-admin question about branch protection / Rulesets configuration), which is unrelated to the submission itself.

### Audit trail of the post-review fixes

| Finding | Severity | Status |
|---|---|---|
| M1 — `README.md` "Pre-implementation" line | Medium | ✅ resolved by PR #10 |
| M2 — `README.md` "(when ready)" demo heading | Medium | ✅ resolved by PR #10 |
| M3 — `SUBMISSION_NOTES.md` `tests (69 passing)` | Medium | ✅ resolved by PR #10 |
| M4 — `IMPLEMENTATION_STATUS.md` fully stale | Medium | ✅ resolved by PR #10 |
| M5 — branch protection / Rulesets repo-admin note | Medium (non-blocking) | ⚠️ not a submission issue |
| L1 — demo summary missing `mock` qualifier | Low | ✅ resolved by PR #10 |
| L2 — `PR_DESCRIPTION.md` is the WIP body of merged PR #1 | Low | ✅ resolved by PR #10 (file deleted) |
