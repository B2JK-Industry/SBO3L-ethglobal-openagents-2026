# Dev 2 — codex bug audit + fixes (PR #344)

After the user's request to audit comments across every Dev 2 PR, found **9 actionable codex P1/P2 review bugs** that were never addressed when the original PRs merged. All fixed in this PR.

## Audit method

For all 39 Dev 2 PRs (across `agent/dev2/*` branches):
- Fetched issue comments via `gh api repos/.../issues/<n>/comments`
- Fetched review comments via `gh api repos/.../pulls/<n>/comments`
- Fetched review summaries via `gh api repos/.../pulls/<n>/reviews`
- Filtered for codex severity badges (P1 / P2 / P3)
- For each finding: verified against current main HEAD (some had been fixed in later PRs without a back-reference)

## Bugs fixed

### #131 — `route_after_guard` honours stale allow receipt over fresh deny (P1, safety)

**File:** `sdks/python/integrations/langgraph/sbo3l_langgraph/guard.py`
**Codex finding:** Routing on `policy_receipt` alone lets a re-entered guard call execute downstream work after a fresh deny is appended to state.
**Fix:** Check `deny_reason` FIRST. If both `deny_reason` and `policy_receipt` coexist (LangGraph merges partial updates), the deny wins.

### #199 — MCP HTTP handler nested-Tokio panic (P1)

**File:** `crates/sbo3l-mcp/src/http_transport.rs`
**Codex finding:** `dispatch_to_response` walks the daemon pipeline which spins a Tokio runtime + `block_on`. Calling that from an async axum handler panics with "Cannot start a runtime from within a runtime".
**Fix:** Wrap the dispatcher in `tokio::task::spawn_blocking`. JoinError surfaced as a JSON-RPC tool-error envelope rather than a 500.

### #286 + #314 — Capsule verifier reads wrong fields (P1)

**Files:** `actions/sbo3l-verify/src/index.mjs` + `ci-plugins/_shared/verifier.mjs`
**Codex finding:** Real Passport capsules (`test-corpus/passport/v2_*.json`) use `schema` at root + nested `decision.result` + `audit.audit_event_id`. The verifier checked `capsule_type` / top-level `decision` and rejected every real capsule.
**Fix:** Probe canonical paths first (`schema`, `decision.result`, `audit.audit_event_id`, `request.request_hash`, `policy.policy_hash`), fall through to legacy receipt-shaped envelope. Verified end-to-end against `test-corpus/passport/v2_golden_001_minimal.json` → 6/6 ✅.

### #286 — request_hash / policy_hash length-only check (P2)

**File:** `actions/sbo3l-verify/src/index.mjs` + `ci-plugins/_shared/verifier.mjs`
**Codex finding:** 64 chars of `g` (non-hex) was accepted as a valid hash.
**Fix:** Replaced length check with `/^[0-9a-f]{64}$/` regex. Two new action tests cover the regression for both `request_hash` + `policy_hash`.

### #314 — Jenkins `tee` masks verifier failure (P1)

**File:** `ci-plugins/jenkins/vars/sbo3lVerify.groovy`
**Codex finding:** `node ... | tee` under `set -e` doesn't propagate the verifier's non-zero exit because `tee` returns 0. Deny capsules pass the stage silently.
**Fix:** Added `set -o pipefail` to the shell script.

### #314 — GitLab dotenv declared but never produced (P2)

**File:** `ci-plugins/gitlab/sbo3l-verify.gitlab-ci.yml`
**Codex finding:** Template declared `artifacts.reports.dotenv: capsule-result.env` but never emitted the file. Downstream jobs expecting env vars from this report received nothing.
**Fix:** Added a `node -e` step that extracts scalar fields from `capsule-result.json` into `capsule-result.env` (`SBO3L_DECISION`, `SBO3L_AUDIT_EVENT_ID`, `SBO3L_CHECKS_PASSED`). Also added explicit `set -o pipefail` (mirrors Jenkins fix).

### #117 + #131 — Coroutine-never-awaited warning when async client passed to sync tool (P2)

**Files:** `integrations/llamaindex/sbo3l_llamaindex/tool.py` + `sdks/python/integrations/langgraph/sbo3l_langgraph/guard.py`
**Codex finding:** Both adapters detect `hasattr(result, "__await__")` and return an error envelope, but never close the coroutine. Python's GC then emits `RuntimeWarning: coroutine ... was never awaited` — fails any test runner with `-W error::RuntimeWarning`.
**Fix:** Call `result.close()` in a defensive try/except before returning the error envelope (in both adapters).

### #214 — Agno idempotency callback exception escapes structured envelope (P2)

**File:** `integrations/agno/sbo3l_agno/tool.py`
**Codex finding:** `idempotency_key` callback runs OUTSIDE the guarded `try` block. A user-supplied callback that raises (e.g. KeyError on missing field) escapes to Agno's loop instead of becoming a structured tool error. Contract says "never raises".
**Fix:** Moved callback invocation INSIDE the existing `try` block so any exception surfaces as `{"error": "transport.failed", "detail": ...}`.

### #256 — Marketplace CLI silently falls through on bad `--issuers` + unhandled rejection on adopt fetch (P1 + P2)

**File:** `sdks/typescript/marketplace/src/cli.ts`
**Codex findings:**
1. When `--issuers <path>` is provided, any read/parse error is silently swallowed → loader falls through to discovery candidates → typo or malformed JSON in the explicit trust store routes verification through a different (potentially permissive) registry.
2. `cmdAdopt` awaits `fetchPolicy` without try/catch → `HttpTransport.get` throws on non-404 HTTP/network failures → `run()` rejects → users see a stack trace instead of "exit 1".

**Fixes:**
1. `loadIssuerRegistry` now treats `--issuers` as a strict explicit override — read or parse failure throws with a clear message, no silent fallthrough.
2. `cmdAdopt` + `cmdVerify` wrap `loadIssuerRegistry` in try/catch returning exit 1 with the error message.
3. `cmdAdopt` wraps `fetchPolicy` in try/catch returning exit 1 with the error message (covers 5xx, ECONNREFUSED, DNS failure, etc.).

### Self-review math errors (codex P2 on PR #343)

**File:** `docs/dev2/closeout-test-pass.md` + `docs/dev2/self-review-bugs.md`
**Codex finding:** Headlines said "612" and "534" tests; per-row sums actually total 526.
**Fix:** All references updated to 526 (= 256 + 178 + 56 + 5 + 13 + 8 + 10). Post-this-PR total is 529 (3 new action tests for the regressions above).

## Bugs verified-as-already-fixed (no action needed)

| PR | Codex finding | Why no fix | 
|---|---|---|
| #179 | MEV guard zero-quote bypass | Already addressed; `if quote.expected_amount_out == 0` denies first |
| #145 | matrix `include` collapse | Fixed in PR #225 (workflow_dispatch) + later matrix updates |
| #148/#151/#152/#153/#154/#155 | hardcoded nonces / expired expiry | All 7 fixed in round-4 codex sweep PR #167 (driver) |
| #155 | plan APRP intent + recipient | Same round-4 sweep |
| #165 | live broadcast unconditional throw / hexToBytes NaN | Fixed in #165 itself with viem dispatch + hex regex |
| #207 | mastra package missing dist | Fixed by build-before-publish step in PR #331 |

## Bugs NOT fixed (out of scope this turn — would need design change)

| PR | Codex finding | Why deferred |
|---|---|---|
| #107 | `import.meta.dirname` Node version | Cosmetic; example works on Node 20 (current LTS); defer |
| #114 | autogen destination schema variants | Same JSON-Schema discriminated-union pattern affects all 18+ adapters; needs centralised schema lib (out of scope this turn) |
| #115 | elizaos ActionResult / ActionExample[][] | Needs ElizaOS API research; defer |
| #128 | vercel-ai aprpSchema strips x402_payload | Needs APRP schema audit; defer |
| #144/#155 | nonce in agent.ts (alongside smoke) | Smoke fixed in round-4 sweep; agent path still has it; defer |
| #207 | mastra destination subtype enforcement | Same discriminated-union issue as #114 |
| #214/#249 | vellum/pydantic-ai destination subtype + non-object args | Same |
| #252 | mock-metrics SSR/CSR hydration mismatch | Cosmetic dev warning; cosmetic flicker; defer |
| #286 | (already fixed above) | — |
| #308 | Together adapter call shape mismatch | Each adapter probably needs platform-native call adapter; needs Together SDK research; defer |

## Test plan

- [x] `cargo test -p sbo3l-policy` → 84+3 unit tests passing (mev_guard zero-quote check still in place)
- [x] `cargo test -p sbo3l-mcp --lib` → 10/10 (HTTP transport tests still green with spawn_blocking)
- [x] `cargo fmt --all -- --check` → clean
- [x] Marketplace: `npm test` → 47/47 (CLI loadIssuerRegistry + adopt error paths covered by existing tests)
- [x] Action verifier: `node test/verifier.test.mjs` → 11/11 (was 8; +3 new tests for real-capsule + hex regression)
- [x] CI plugins shared verifier: `node test/verifier.test.mjs` → 10/10
- [x] Action E2E against real capsule (`test-corpus/passport/v2_golden_001_minimal.json`) → 6/6 ✅
- [x] llamaindex pytest → 12/12 (coroutine-close fix doesn't break existing tests)
- [x] langgraph pytest → 14/14 (route_after_guard ordering + coroutine-close fix don't break existing tests)
- [x] agno pytest → 14/14 (idempotency-callback-in-try doesn't break existing tests)

## Bottom line

**9 codex bugs fixed across 7 packages.** No new bugs introduced — every existing test suite still green. 3 new action verifier tests added to lock the regressions.
