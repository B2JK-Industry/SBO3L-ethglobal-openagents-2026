# SBO3L × Uniswap — partner one-pager

> A credible safety layer for agentic swaps, not just another quote API
> call.

**Status: live read-side QuoterV2 against Sepolia shipped + verified.
Universal Router + Smart Wallet abstraction + MEV guard merged. Mainnet
swap envelope CLI shipped. Live broadcast remains env-gated +
intentionally not exercised in CI.**

## The pitch in one paragraph

Autonomous agents should not be able to swap any token to any recipient
at any slippage. SBO3L sits in front of the agent and turns Uniswap
interaction into policy-controlled finance: the agent emits a swap intent
through APRP, a swap-policy guard enforces token allowlists, max
notional, max slippage, quote freshness, and treasury recipient, and
SBO3L's policy boundary independently denies if any of those checks
fail. Approved quotes are routed to the executor; denied quotes still
produce signed deny receipts for auditability, but they die at the policy
boundary and never reach the executor.

## What is implemented today (on `main`, this build)

- Adapter trait and `UniswapExecutor`
  (`crates/sbo3l-execution/src/uniswap.rs`) with two constructors:
  - `UniswapExecutor::local_mock()` — CI default. Returns a
    deterministic `uni-<ULID>` execution_ref against a stored quote
    fixture.
  - `UniswapExecutor::live_from_env()` — **shipped + verified**. Calls
    `quoteExactInputSingle()` on the Sepolia QuoterV2 contract
    ([`0xEd1f6473345F45b75F8179591dd5bA1888cf2FB3`](https://sepolia.etherscan.io/address/0xEd1f6473345F45b75F8179591dd5bA1888cf2FB3))
    via JSON-RPC, env-gated on `SBO3L_UNISWAP_RPC_URL` +
    `SBO3L_UNISWAP_TOKEN_OUT`. Returns real `amountOut` +
    `sqrtPriceX96After` + `initializedTicksCrossed` + `gasEstimate`
    into the Passport capsule's `executor_evidence`. The bare
    back-compat `UniswapExecutor::live()` ctor (no transport, no
    config) returns `BackendOffline` at runtime — that's the legacy
    surface the live broadcast path will replace.
- **Universal Router integration** (PR [#171](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/171))
  — multi-hop route parsing + V3 swap construction shipped. Source:
  [`crates/sbo3l-execution/src/uniswap_trading.rs`](../../crates/sbo3l-execution/src/uniswap_trading.rs).
- **Smart Wallet abstraction** (PR [#183](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/183))
  — wraps the recipient as a smart-account session for
  `smart_account_session` APRP variants.
- **MEV guard** (PR [#179](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/179),
  10 unit tests) — three layers of slippage defense (slippage cap +
  recipient allowlist + private mempool requirement). Zero-quote
  bypass codex finding fixed in same PR. Source:
  [`crates/sbo3l-policy/src/mev_guard.rs`](../../crates/sbo3l-policy/src/mev_guard.rs).
- **Mainnet swap envelope CLI** (PR [#394](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/394))
  — `sbo3l swap envelope` builds the mainnet-shaped Universal Router
  payload + signed APRP envelope ready for broadcast. Broadcast
  itself is intentionally a separate human-gated step.
- Swap-policy guard `evaluate_swap` (`crates/sbo3l-execution/src/uniswap.rs`)
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
- **SBO3L Passport capsule — Uniswap quote evidence (P6.1, shipped).**
  `sbo3l passport run --executor uniswap` emits a
  `sbo3l.passport_capsule.v1` JSON whose `execution.executor_evidence`
  slot carries the 10-field `UniswapQuoteEvidence` payload (`quote_id`,
  `quote_source`, `input_token`, `output_token`, `route_tokens`,
  `notional_in`, `slippage_cap_bps`, `quote_timestamp_unix`,
  `quote_freshness_seconds`, `recipient_address`). The capsule
  round-trips through `sbo3l passport verify` (exit 0). Source:
  [`crates/sbo3l-execution/src/uniswap.rs`](../../crates/sbo3l-execution/src/uniswap.rs);
  demo step:
  [`demo-scripts/sponsors/uniswap-guarded-swap.sh`](../../demo-scripts/sponsors/uniswap-guarded-swap.sh).
- Builder feedback (current): [`FEEDBACK.md` §Uniswap](../../FEEDBACK.md).

## What is target (SBO3L Passport phase, not on main yet)

These are explicit *targets* documented for the team and for Uniswap
reviewers — none of them are claimed as shipped:

- **Live broadcast of a signed swap envelope to mainnet** — the
  envelope-build path is shipped (PR #394); actually broadcasting to
  mainnet is intentionally a separate human-gated step (operator
  signs + broadcasts via their own wallet/relayer; SBO3L never holds
  mainnet keys in CI). The envelope's `sbo3l_*` fields are already
  what the broadcaster needs to prove upstream authorization.
- **Signed-quote anchoring (target ask)** — when the Trading API
  publishes server-issued `quote_id` + `expires_at` + canonical
  quote hash, SBO3L would anchor that hash into the decision token
  so a downstream executor can require the same quote. See feedback
  below. Today freshness is approximated from local timestamps.

## Why Uniswap specifically

Quote shapes (input / output amount, route, slippage) map cleanly onto
SBO3L's `smart_account_session` APRP variant. The split between
swap-policy guard (numeric / policy checks against the canonical quote)
and SBO3L's recipient / provider / budget boundary is natural — they
catch overlapping problems at different layers. Defense in depth: the
rug-quote fixture is denied independently by both the swap-policy guard
(`output_token_allowlisted` first, `max_slippage_bps` and
`treasury_recipient_allowlisted` as additional violations) and SBO3L's
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

- SBO3L **does not** broadcast a real swap to mainnet from CI in this
  build — that's intentionally human-gated. What it does call live is
  the Sepolia QuoterV2 contract (`0xEd1f6473345F45b75F8179591dd5bA1888cf2FB3`)
  via JSON-RPC for real read-side `quoteExactInputSingle`, env-gated
  on `SBO3L_UNISWAP_RPC_URL` + `SBO3L_UNISWAP_TOKEN_OUT` — verified
  end-to-end during the submission window. The mainnet swap envelope
  CLI (`sbo3l swap envelope`) builds + signs the broadcast-ready
  payload but does not transmit. The demo default uses
  `UniswapExecutor::local_mock()` against the fixture catalogue.
- The mock `uni-<ULID>` execution_ref **is not** a real Uniswap
  transaction id; live mode emits real QuoterV2 return values
  (`amountOut`, `sqrtPriceX96After`, `initializedTicksCrossed`,
  `gasEstimate`) into the Passport capsule's `executor_evidence`.
- This is **not** a Uniswap v4 hook project. SBO3L is a policy /
  authorisation layer that sits in front of Uniswap, not a DEX hook.
- The Uniswap quote-evidence section of the SBO3L Passport capsule is
  **shipped** (P6.1) — `executor_evidence` is a non-empty 10-field
  object on the allow path, omitted on the deny path. The transport-
  level `live_evidence` slot still stays `null` in mock mode (and the
  verifier still rejects mock-with-evidence in either slot direction);
  P6.1 only adds the new mode-agnostic `executor_evidence` slot.

## Pointers in this repo

- Adapter + swap-policy guard source: [`crates/sbo3l-execution/src/uniswap.rs`](../../crates/sbo3l-execution/src/uniswap.rs)
- Sponsor demo (mock today): [`demo-scripts/sponsors/uniswap-guarded-swap.sh`](../../demo-scripts/sponsors/uniswap-guarded-swap.sh)
- Production-shaped catalogue + transition guide: [`demo-fixtures/mock-uniswap-quotes.json`](../../demo-fixtures/mock-uniswap-quotes.json) / [`demo-fixtures/mock-uniswap-quotes.md`](../../demo-fixtures/mock-uniswap-quotes.md)
- Production transition checklist (env vars / endpoints / credentials): [`docs/production-transition-checklist.md` §Uniswap](../production-transition-checklist.md#uniswap-guarded-swap)
- Builder feedback: [`FEEDBACK.md` §Uniswap](../../FEEDBACK.md)
- Product source of truth: [`docs/product/SBO3L_PASSPORT_SOURCE_OF_TRUTH.md`](../product/SBO3L_PASSPORT_SOURCE_OF_TRUTH.md)
- Reward strategy: [`docs/product/REWARD_STRATEGY.md`](../product/REWARD_STRATEGY.md)
