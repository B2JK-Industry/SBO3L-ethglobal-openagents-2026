# Mandate × Uniswap — partner one-pager

> A credible safety layer for agentic swaps, not just another quote API
> call.

**Status: target product framing, with the parts that already exist on
`main` clearly separated from the parts that depend on later phases.**

## The pitch in one paragraph

Autonomous agents should not be able to swap any token to any recipient
at any slippage. Mandate sits in front of the agent and turns Uniswap
interaction into policy-controlled finance: the agent emits a swap intent
through APRP, a swap-policy guard enforces token allowlists, max
notional, max slippage, quote freshness, and treasury recipient, and
Mandate's policy boundary independently denies if any of those checks
fail. Approved quotes are routed to the executor; denied quotes still
produce signed deny receipts for auditability, but they die at the policy
boundary and never reach the executor.

## What is implemented today (on `main`, this build)

- Adapter trait and `UniswapExecutor`
  (`crates/mandate-execution/src/uniswap.rs`) with two constructors:
  - `UniswapExecutor::local_mock()` — used in every demo path today.
    Returns a deterministic `uni-<ULID>` execution_ref against a stored
    quote fixture.
  - `UniswapExecutor::live()` — present as a constructor; intentionally
    `BackendOffline` until a stable Trading API endpoint and credentials
    are wired. **No live network call is made in this build.**
- Swap-policy guard `evaluate_swap` (`crates/mandate-execution/src/uniswap.rs`)
  enforces, in field order:
  1. `input_token_allowlisted`
  2. `output_token_allowlisted`
  3. `max_notional_usd`
  4. `max_slippage_bps`
  5. `quote_freshness`
  6. `treasury_recipient_allowlisted`
- Three-quote production-shaped catalogue:
  [`demo-fixtures/mock-uniswap-quotes.json`](../../demo-fixtures/mock-uniswap-quotes.json)
  with companion guide
  [`demo-fixtures/mock-uniswap-quotes.md`](../../demo-fixtures/mock-uniswap-quotes.md):
  1. `happy_path_within_caps` — bounded USDC → ETH; allowed.
  2. `multiple_violations_rug_quote` — USDC → RUG with 1500 bps
     slippage and an off-allowlist recipient. Trips three checks at
     once. Documented `expected_swap_policy_reason` is the **first**
     violation under field-order traversal (`output_token_allowlisted`),
     with `expected_additional_violations` listing the other two.
  3. `recipient_allowlist_violation` — bounded slippage, off-allowlist
     recipient is the **only** failing check.
- Per-quote runtime mocks (gate 9 of the 13-gate demo):
  [`demo-fixtures/uniswap/quote-USDC-ETH.json`](../../demo-fixtures/uniswap/quote-USDC-ETH.json)
  and
  [`demo-fixtures/uniswap/quote-USDC-RUG.json`](../../demo-fixtures/uniswap/quote-USDC-RUG.json).
- Builder feedback (current): [`FEEDBACK.md` §Uniswap](../../FEEDBACK.md).

## What is target (Mandate Passport phase, not on main yet)

These are explicit *targets* documented for the team and for Uniswap
reviewers — none of them are claimed as shipped:

- **Mandate Passport capsule (target)** — a single JSON artefact
  (`mandate.passport_capsule.v1`, schema/verifier owned by the A-side
  Passport CLI work) that records, in one file, the request, the
  decision, the swap-policy result vector, and the executor's
  `execution_ref`. Until the capsule schema lands on `main`, no UI
  claims a Uniswap-evidence capsule was produced.
- **Capsule quote-evidence section (target)** — when capsule schema
  lands, the Uniswap path will populate:
  - quote id / source;
  - input / output token symbols;
  - route token list (when surfaced by the API);
  - notional + caps;
  - slippage + caps;
  - quote timestamp + freshness result;
  - recipient check;
  - per-violation deny reasons when denied.
- **Live Trading API call (future, gated)** — wired through
  `UniswapExecutor::live()` and exposed via an explicit
  `MANDATE_UNISWAP_LIVE=1` env-var gate, never as a silent fallback from
  mock. CI will never require it. **No live Uniswap API call is made in
  this build.**
- **Signed-quote anchoring (target ask, not implemented)** — when the
  Trading API publishes server-issued `quote_id` + `expires_at` +
  canonical quote hash, Mandate would anchor that hash into the decision
  token so a downstream executor can require the same quote. See
  feedback below.

## Why Uniswap specifically

Quote shapes (input / output amount, route, slippage) map cleanly onto
Mandate's `smart_account_session` APRP variant. The split between
swap-policy guard (numeric / policy checks against the canonical quote)
and Mandate's recipient / provider / budget boundary is natural — they
catch overlapping problems at different layers. Defense in depth: the
rug-quote fixture is denied independently by both the swap-policy guard
(`output_token_allowlisted` first, `max_slippage_bps` and
`treasury_recipient_allowlisted` as additional violations) and Mandate's
policy boundary (`policy.deny_recipient_not_allowlisted`).

## What we are asking Uniswap for (concrete, scoped)

These are the same asks recorded in
[`FEEDBACK.md` §Uniswap](../../FEEDBACK.md), summarised here:

1. **Signed quotes.** Server-issued `quote_id`, `expires_at`, and a
   canonical quote hash, so a policy engine can anchor the hash into a
   decision token and a downstream executor can require the same quote.
2. **`expires_at` on the quote response.** Today freshness is
   approximated from local timestamps; the demo's static fixture has to
   relax this and we surface a `(relaxed)` flag so judges see it. A
   server-side `expires_at` removes the approximation.
3. **Route token enumeration.** Today multi-hop routes occasionally
   touch tokens not on a caller's allowlist. Returning every intermediate
   token (not just input / output) lets policy engines opt in or out at
   the path level instead of denying by default.
4. **First-class slippage caps in the request.** Letting the API reject a
   quote whose realised slippage already exceeds the caller's cap removes
   a class of agent footguns at the network boundary.
5. **Canonical quote hash.** A documented JCS-shape over the quote so
   third-party policy engines can hash deterministically without inventing
   a canonicalisation.

## What this one-pager will NOT claim

- Mandate **does not** call the Uniswap Trading API in this build.
  `UniswapExecutor::live()` is a `BackendOffline` stub today; every demo
  path uses `UniswapExecutor::local_mock()` against the fixture catalogue.
- The mock `uni-<ULID>` execution_ref **is not** a real Uniswap
  transaction id.
- This is **not** a Uniswap v4 hook project. Mandate is a policy /
  authorisation layer that sits in front of Uniswap, not a DEX hook.
- Mandate Passport capsule + Uniswap quote-evidence section are **target
  product framing**, not shipped artefacts in this build. The capsule
  schema (A-side) lands in a later phase; this one-pager will be updated
  to reference the actual schema once that PR is on `main`.

## Pointers in this repo

- Adapter + swap-policy guard source: [`crates/mandate-execution/src/uniswap.rs`](../../crates/mandate-execution/src/uniswap.rs)
- Sponsor demo (mock today): [`demo-scripts/sponsors/uniswap-guarded-swap.sh`](../../demo-scripts/sponsors/uniswap-guarded-swap.sh)
- Production-shaped catalogue + transition guide: [`demo-fixtures/mock-uniswap-quotes.json`](../../demo-fixtures/mock-uniswap-quotes.json) / [`demo-fixtures/mock-uniswap-quotes.md`](../../demo-fixtures/mock-uniswap-quotes.md)
- Production transition checklist (env vars / endpoints / credentials): [`docs/production-transition-checklist.md` §Uniswap](../production-transition-checklist.md#uniswap-guarded-swap)
- Builder feedback: [`FEEDBACK.md` §Uniswap](../../FEEDBACK.md)
- Product source of truth: [`docs/product/MANDATE_PASSPORT_SOURCE_OF_TRUTH.md`](../product/MANDATE_PASSPORT_SOURCE_OF_TRUTH.md)
- Reward strategy: [`docs/product/REWARD_STRATEGY.md`](../product/REWARD_STRATEGY.md)
