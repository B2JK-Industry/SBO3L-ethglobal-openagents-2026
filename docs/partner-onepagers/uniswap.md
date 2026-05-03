# SBO3L × Uniswap — partner one-pager

> A credible safety layer for agentic swaps, not just another quote API
> call.

**Status: target product framing, with the parts that already exist on
`main` clearly separated from the parts that depend on later phases.**

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
  (`crates/sbo3l-execution/src/uniswap.rs`) with three constructors:
  - `UniswapExecutor::local_mock()` — default in CI / demo path.
    Returns a deterministic `uni-<ULID>` execution_ref against a stored
    quote fixture.
  - `UniswapExecutor::live_from_env()` — **shipped + verified live**:
    hits Sepolia QuoterV2 (`0xEd1f6473345F45b75F8179591dd5bA1888cf2FB3`)
    via JSON-RPC for a real `quoteExactInputSingle`, env-gated on
    `SBO3L_UNISWAP_RPC_URL` + `SBO3L_UNISWAP_TOKEN_OUT`. Real QuoterV2
    return values (`amountOut`, `sqrtPriceX96After`, `initializedTicksCrossed`,
    `gasEstimate`) populate the Passport capsule's `executor_evidence`.
  - `UniswapExecutor::live()` — bare back-compat ctor; returns
    `BackendOffline` at runtime when no env config is set (use
    `live_from_env()` instead).
- **Mainnet broadcast UNI-A1 (LIVE):** A real Uniswap V3 swap was
  broadcast on Ethereum mainnet from the SBO3L deploy wallet
  (`0xdc7EFA…D231`) — 0.005 ETH → 11.5743 USDC, settled in block
  25,013,950, gas 139,971 @ 2.19 gwei. Tx hash:
  [`0xed68d1301b479c4229bc89cca5162b56517b80cbaeb654323e05b183000aff0b`](https://etherscan.io/tx/0xed68d1301b479c4229bc89cca5162b56517b80cbaeb654323e05b183000aff0b).
  The same swap-policy guard (token allowlist + slippage + treasury
  recipient) protects this and any future agent-initiated swap.
- **Mainnet swap envelope CLI** `sbo3l uniswap swap` (PR #394) — builds
  + optionally broadcasts a `sbo3l.uniswap_swap_envelope.v1` JSON
  artefact for V3 `exactInputSingle` swaps on either Sepolia or mainnet.
  Default `--dry-run`; `--broadcast` requires the `eth_broadcast` Cargo
  feature plus `SBO3L_ALLOW_MAINNET_TX=1` for `--network mainnet`.
- **Universal Router** (PR #171), **Smart Wallet abstraction** (PR #183),
  **MEV guard** (PR #179) all shipped in `crates/sbo3l-execution/`.
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

## What is target (post-submission roadmap)

These are explicit *targets* — none claimed as shipped:

- **Mainnet Universal Router integration via Trading API.** Today's
  mainnet UNI-A1 broadcast goes through Universal Router directly via
  envelope CLI; a server-side Trading API integration would simplify
  fee-tier discovery + signed-quote handling.
- **Signed-quote anchoring** — when the Trading API publishes
  server-issued `quote_id` + `expires_at` + canonical quote hash, SBO3L
  would anchor that hash into the decision token so a downstream
  executor can require the same quote. See feedback below.
- **v4 hook integration** — SBO3L is a policy boundary, not a DEX hook,
  but a "policy-bounded swap" v4 hook reference (input/output token
  allowlist, slippage cap, recipient guard as Solidity library) would
  make every framework SDK adopter ship the same guarantees by default.

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

- SBO3L **does not** call the Uniswap Trading API (server-side REST) in this build — what it calls live is (a) the Sepolia QuoterV2 contract `0xEd1f6473345F45b75F8179591dd5bA1888cf2FB3` via JSON-RPC for a real read-side `quoteExactInputSingle`, and (b) the mainnet Universal Router `0x4c82d1fbfe28c977cbb58d8c7ff8fcf9f70a2cca` via the UNI-A1 mainnet broadcast (tx `0xed68d1…aff0b`). The demo default still uses `UniswapExecutor::local_mock()` against the fixture catalogue.
- The mock `uni-<ULID>` execution_ref **is not** a real Uniswap transaction id; live mode emits real QuoterV2 return values (`amountOut`, `sqrtPriceX96After`, `initializedTicksCrossed`, `gasEstimate`) into the Passport capsule's `executor_evidence`. The mainnet UNI-A1 broadcast emits a real Etherscan-verifiable tx hash.
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
