# Final Review — Mandate, ETHGlobal Open Agents 2026

**Reviewed commit:** `f52596c433861c72c2f22ffe183674524d45e14d`
**Date:** 2026-04-28
**Scope:** Read-only audit of `main` after merging PRs #5, #6, #7, #8, #9.
**Status (TL;DR):** **READY_AFTER_FIXES** at the time of review — only doc-only corrections needed; no code, security or demo blockers. The medium-severity findings (M1–M4) are addressed in the docs-only PR `docs: finalize ETHGlobal submission readiness`. After that PR merges, the project is **READY_FOR_SUBMISSION**.

---

## 1. Repository state

| Check | Result |
|---|---|
| Local tree clean | ✅ `git status -sb` empty |
| Open PRs | 0 |
| Latest CI on `main` | ✅ success (`refactor: deduplicate same_origin into mandate-policy::util (#5)`, run 25040118315, 29s) |
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
- `mandate-core` — 27 (APRP, hashing, signer, receipt, decision_token, audit, schema)
- `mandate-policy` — 21 + 13 + 5 across `model` / `engine` / `expr` / `budget` / `util`
- `mandate-storage` — 4 (audit_append + verify, audit_last × 2, migrations idempotent)
- `mandate-server` — 6 (legit / prompt-injection / adversarial / replay × 3)
- `mandate-identity` — 3
- `mandate-execution` — 3 (KeeperHub) + Uniswap suite

---

## 3. Security & correctness review

Methodology: an Explore subagent gathered evidence with file paths and line ranges; I spot-checked the most security-relevant claims (nonce gate placement, freeze_all rule, agent-side keys, validate() body) directly. All claims hold.

| Area | Verdict | Key evidence |
|---|---|---|
| 1. APRP schema strictness | **PASS** | `schemas/aprp_v1.json:6` `additionalProperties: false`; `crates/mandate-core/src/aprp.rs:7` `#[serde(deny_unknown_fields)]` (and on every nested struct + enum); test `unknown_field_is_rejected_by_serde` covers the adversarial fixture. |
| 2. Canonical hashing | **PASS** | `crates/mandate-core/src/hashing.rs:14` JCS via `serde_json_canonicalizer`; `golden_aprp_hash_is_locked` test pins `c0bd2fab…`. |
| 3. Nonce replay protection | **PASS** | `crates/mandate-server/src/lib.rs:181-194` gate before `request_hash`/policy/budget/audit/sign. Replay key is `aprp.nonce` only. HTTP 409 + `protocol.nonce_replay`. Three regression tests: replay-rejected, distinct-nonces-OK, mutated-body-same-nonce-still-rejected (no `receipt`/`audit_event_id` in response). |
| 4. Policy validation | **PASS** | `crates/mandate-policy/src/model.rs:214-269` validates all five uniqueness invariants (agent_id, rule.id, provider.id, recipient `(addr.lc, chain)`, budget `(agent_id, scope, scope_key)`). Both `parse_json` and `parse_yaml` route through `validate()`. |
| 5. Policy decisions | **PASS** | Allow returns `matched_rule_id`; deny returns deterministic `deny_code`. `deny-emergency-freeze` rule in `test-corpus/policy/reference_low_risk.json:55-59` triggers on `input.emergency.freeze_all == true`. `null` semantics: `==` identity-true/false, `<` errors. `same_origin` rejects `example.com.attacker.com` substring trap. |
| 6. Budget semantics | **PASS** | `per_tx` does not accumulate (`commit()` skips `BudgetScope::PerTx`); daily/monthly/per_provider do. `commit()` runs only after Allow. Replays are rejected before reaching budget check (nonce gate is upstream). |
| 7. Audit chain | **PASS** | `audit_append` chains via `prev_event_hash` → `event_hash`. `audit_verify` walks chain + verifies signatures. Post-PR-#6: `audit_last` returns `Ok(None)` only on `QueryReturnedNoRows`; other SQLite errors propagate. |
| 8. Receipts & signatures | **PASS** | Ed25519 over canonical JSON for receipts, decision tokens, audit events. Tampering tests assert verification failure. **No private keys committed.** Dev signing seeds are deterministic constants in `mandate-server/src/lib.rs:51-65` explicitly labelled `⚠ DEV ONLY ⚠` with a `with_signers()` injection path for production. |
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
| What is mocked? | (1) ENS testnet resolver — uses local fixture; the trait abstraction is real. (2) KeeperHub backend — `local_mock()` when `MANDATE_KEEPERHUB_LIVE != 1`. (3) Uniswap quote — static fixture when `MANDATE_UNISWAP_LIVE != 1`. (4) Signing seeds — deterministic dev seeds, gated for production via `with_signers()`. |
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
| Project name consistently `Mandate` / `mandate` | ✅ verified across all user-facing docs |
| Old names (`Agent Vault OS`, `Vault 402`) absent from user-facing submission text | ✅ only appear in spec docs and as the *historical* name `agent-vault-os` of the planning repo (intentional, transparent attribution) |

---

## 6. Findings

### Blockers
**None.**

### High severity
**None.**

### Medium severity (doc-only; misleading to judges)

- **M1.** `README.md:13` — `**Status:** Pre-implementation. Repo bootstrap in progress.` — false. Implementation is complete; 90 tests pass; demo green. A judge landing on README first will mistakenly think the project isn't built.
  - **Recommended fix:** replace with `Implementation complete; demo green end-to-end on commit ` `<sha>`. See `IMPLEMENTATION_STATUS.md`.
- **M2.** `README.md:22` — heading `## How to run the demo (when ready)` — implies demo isn't ready.
  - **Recommended fix:** drop `(when ready)`.
- **M3.** `SUBMISSION_NOTES.md:28` — `CI: fmt, clippy, tests (69 passing), schema validation.` — stale. Actual is 90.
  - **Recommended fix:** update to `tests (90 passing)`.
- **M4.** `IMPLEMENTATION_STATUS.md` is fully stale: lists 5 PRs as open, names PR #7 as the current branch, says "Wait for Daniel's manual review on PR #7 before any merge", cites 71 tests. All five PRs are merged and tests are 90.
  - **Recommended fix:** rewrite the file as a post-merge wrap-up: PRs merged, final test count, demo status, no blockers.
- **M5.** *(Repo-admin note, not a submission issue.)* The classic branch-protection API for `main` returns HTTP 404. This may simply mean protection is configured via the newer **GitHub Rulesets** mechanism (separate API at `repos/{owner}/{repo}/rulesets`) rather than classic branch protection — the two are often confused. **Not a code or submission blocker.** If neither is configured, enabling either before broader open-source adoption is a fine post-hackathon repo-admin task.

### Low severity (polish)

- **L1.** Demo final summary uses `Legitimate x402 spend approved -> KeeperHub executed.` — could be clearer that this is the local-mock executor, since the same line for Uniswap doesn't carry that nuance either. Mid-flow output makes the mock context explicit, but a reader skimming only the final summary might miss it. Optional one-word edit (`KeeperHub executed (mock)` and `swap allowed (mock)`).
- **L2.** `PR_DESCRIPTION.md` is the body of merged PR #1 and still reads `[WIP] / Draft / not ready to merge / 23 tests pass`. Not user-facing for the ETHGlobal submission, but inconsistent with reality. Could be deleted or refreshed; safest to delete since GitHub already preserves the PR history.

---

## 7. Recommended fixes before submission

In priority order:

1. **Fix M1, M2, M3** — three small line edits across `README.md` and `SUBMISSION_NOTES.md`. Pure doc; no code touched.
2. **Fix M4** — rewrite `IMPLEMENTATION_STATUS.md` as a post-merge snapshot.
3. **Fix L1** — optional polish on demo final summary lines (one-line edit in `demo-scripts/run-openagents-final.sh`); only if you want belt-and-braces honesty for the video viewer.
4. **(Post-submission)** **M5** — enable branch protection on `main`.
5. **(Optional)** **L2** — delete or refresh `PR_DESCRIPTION.md`.

Items 1–3 are doc-only. Items 4 and 5 are not changes to this commit's submission readiness.

---

## 8. Final verdict

# READY_AFTER_FIXES

The codebase is solid: build, tests, demo all green; all nine security/correctness areas pass with code-level evidence; mocks are honestly labelled. The only outstanding issues are **stale documentation strings** that would mislead a judge skimming README/SUBMISSION_NOTES. Fixing M1–M4 takes minutes and pushes the project to **READY_FOR_SUBMISSION**.
