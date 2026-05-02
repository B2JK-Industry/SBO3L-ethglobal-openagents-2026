# Dev 2 self-review — bugs found in own closeout reports (PR #338)

Written 2026-05-02 after re-auditing the round-15 closeout work. Five real bugs found, all in the closeout reports themselves (not the shipped code). Each is corrected in the same PR that ships this document.

## Bug 1 — Wrong package count in `closeout-status.md`

**Claimed:** "30 framework adapters (TypeScript + Python)"
**Actual:** 32 publishable packages — 25 npm + 7 PyPI

**Audit evidence:**
- `find . -name "package.json" | xargs grep -l '"name": "@sbo3l/'` returns **25** files under `integrations/` + `sdks/typescript/integrations/` + `sdks/typescript/marketplace` + `sdks/typescript`
- `find . -name "pyproject.toml" | xargs grep -l '^name = "sbo3l-'` returns **7** files under `integrations/` + `sdks/python` + `sdks/python/integrations/langgraph`

The original count missed two npm packages: `@sbo3l/sdk` (the root SDK itself) and `@sbo3l/marketplace` (Phase 3.3 SDK). Both are real publishable packages, not just internal modules.

**Fix:** `docs/dev2/closeout-status.md` updated to "32 publishable packages (25 npm + 7 PyPI)" with the corrected per-package table.

## Bug 2 — Stale npm publish state in `npm-publishes-final.md`

**Claimed:** "8/30 live; 22 pending npm + 2 pending PyPI"
**Actual at audit time:** **30/32 live** — only `sbo3l-agno` + `sbo3l-pydantic-ai` (PyPI) still pending

**What happened:** Daniel ran `gh workflow run integrations-publish.yml` (or pushed tags) for the post-cohort packages between when I wrote the closeout report and the post-merge audit. Every npm package I had marked as "pending NPM_TOKEN" is now live. Per-package re-probe via `https://registry.npmjs.org/<encoded-pkg>/1.2.0`:

| Package | claimed (closeout) | actual (now) |
|---|---|---|
| `@sbo3l/sdk` | ❌ NPM_TOKEN missing | ✅ live |
| `@sbo3l/marketplace` | (not listed) | ✅ live |
| `@sbo3l/vercel-ai` | ❌ publish failure | ✅ live |
| `@sbo3l/openai-assistants` | ❌ no tag pattern | ✅ live |
| `@sbo3l/anthropic` | ❌ no tag pattern | ✅ live |
| `@sbo3l/anthropic-computer-use` | ❌ no tag pattern | ✅ live |
| `@sbo3l/mastra` | ❌ no tag pattern | ✅ live |
| `@sbo3l/vellum` | ❌ no tag pattern | ✅ live |
| `@sbo3l/langflow` | ❌ no tag pattern | ✅ live |
| `@sbo3l/inngest` | ❌ no tag pattern | ✅ live |
| `@sbo3l/letta` | ❌ no tag pattern | ✅ live |
| `@sbo3l/autogpt` | ❌ no tag pattern | ✅ live |
| `@sbo3l/babyagi` | ❌ no tag pattern | ✅ live |
| `@sbo3l/superagi` | ❌ no tag pattern | ✅ live |
| `@sbo3l/cohere-tools` | ❌ no tag pattern | ✅ live |
| `@sbo3l/together` | ❌ no tag pattern | ✅ live |
| `@sbo3l/perplexity` | ❌ no tag pattern | ✅ live |
| `@sbo3l/replicate` | ❌ no tag pattern | ✅ live |
| `@sbo3l/modal` | ❌ no tag pattern | ✅ live |
| `@sbo3l/e2b` | ❌ no tag pattern | ✅ live |
| `@sbo3l/agentforce` | ❌ no tag pattern | ✅ live |
| `@sbo3l/copilot-studio` | ❌ no tag pattern | ✅ live |

**Fix:** `docs/proof/npm-publishes-final.md` rewritten with the 30/32 live state.

## Bug 3 — Math error in `npm-publishes-final.md` tally row

**Claimed:**
```
| npm | **3** | 19 | 22 |
```

**Actual:** 3 + 22 = 25, not 22. The "Total" column should equal "Live" + "Pending". I listed 22 packages in the per-package "pending" table but typed 22 again in the total — typo / copy-paste error.

**Fix:** Corrected to `| npm | 25 | 0 | 25 |` reflecting current state.

## Bug 4 — Misleading framing in `closeout-test-pass.md`

**Claimed:** "20/20 adapters PASS — 256 tests"
**Actual scope:** 20 packages from `sdks/typescript/integrations/` ONLY. The 5 packages **outside** that directory weren't tested: `@sbo3l/sdk`, `@sbo3l/marketplace`, and the 3 cohort npm packages at top-level `integrations/` (autogen, elizaos, langchain-typescript).

The 20/20 number is technically correct for what the script ran, but the headline "all adapters pass" framing oversells it. The script's `find sdks/typescript/integrations -mindepth 1 -maxdepth 1` filter explicitly excluded the other 5.

**Fix:** Re-ran an extended test pass for the missing 5. Results below; all green:

| Package | Path | Tests |
|---|---|---|
| `@sbo3l/sdk` | `sdks/typescript/` | 97 |
| `@sbo3l/marketplace` | `sdks/typescript/marketplace/` | 47 |
| `@sbo3l/autogen` | `integrations/autogen/` | 9 |
| `@sbo3l/elizaos` | `integrations/elizaos/` | 14 |
| `@sbo3l/langchain` | `integrations/langchain-typescript/` | 11 |

That's **178 additional vitest** the closeout report missed.

**Updated total Dev 2 TypeScript tests:**

| Suite | Tests |
|---|---|
| 25 npm packages (full audit, was 20) | 434 (was 256) |
| 3 ChatOps bots | 56 |
| Cross-protocol killer demo | 5 + 7-check verifier |
| Observability dashboard | 13 |
| GitHub Action verifier | 8 |
| CI plugins shared verifier | 10 |
| **Total Dev 2 TypeScript tests** | **526** (was 402; per-row math: 434 + 56 + 5 + 13 + 8 + 10) |

**Fix:** `docs/dev2/closeout-test-pass.md` updated to clarify scope + add the 5 missing packages with a 25/25 result.

## Bug 5 — `scope-cuts-r13-r14.md` references stale "8/30 packages live"

Same root cause as Bug 2 — written before the post-cohort publishes ran. Now corrected to 30/32 live + 2 pending PyPI publishers (`pypi-agno-py`, `pypi-pydantic-ai-py`).

## Code-level audit (no bugs found)

I also re-read every shipped TS/Rust file for runtime bugs. Spot checks:

- `examples/cross-protocol-killer/src/agent.ts::chainLinksConsistent` — loop runs `i=1..N-1`, comparing `transcript[i].prev_audit_event_id` to `transcript[i-1].audit_event_id`. Step 1 (i=0) correctly skipped. Match between agent.ts in-demo verifier + verify-output.ts offline verifier.
- `apps/chatops-slack/src/server.ts::verifySlackSignature` — length-checks before `timingSafeEqual` (which throws on length mismatch). 5-min replay window. Falls open ONLY when secret unset (dev mode). Production safe per DEPLOY.md.
- `apps/chatops-discord/src/server.ts::verifyDiscordSignature` — `nacl.sign.detached.verify(message, sig, pub)` with `message = ts || rawBody` per Discord docs. Handles missing-headers + parse failure.
- `crates/sbo3l-server/src/lib.rs::admin_metrics` — `started_at_iso` derives wall-clock start from `Instant::now() - elapsed` (1-sec precision; acceptable). 503 fallback shape mirrors `/v1/healthz`.
- `actions/sbo3l-verify/src/index.mjs` — `shouldComment` boolean guards each side-effect; `STEP_SUMMARY` / `STEP_OUTPUT` writes guarded with truthy check; PR comment failures non-fatal.
- `ci-plugins/_shared/verifier.mjs` — early-return on `!isObj(c)` returns `{decision, checks}` without `audit_event_id` field; safe because consumer always handles `undefined` via `if (result.audit_event_id)`.
- `sdks/typescript/marketplace/src/cli.ts::cmdAdopt` — re-hashes registry-returned content vs requested `policy_id` (independent of signature) — verified by test `adopt refuses bundle whose policy_id ≠ content hash`.

**No code-level regressions found in this audit.** All 5 bugs are in reporting accuracy, not shipped functionality.

## Bottom line

5 bugs, all in closeout documentation. None affect runtime behavior of any shipped package or app. All corrected in this PR.

The actual final state is **better** than what the original closeout claimed — 30/32 packages live (originally claimed 8/30), 526 tests passing (originally claimed 402).
