# `mock-uniswap-quotes.json` — production-shaped Uniswap quote catalogue

A catalogue of three deterministic Uniswap quote envelopes shaped like
what the Mandate Uniswap swap-policy guard
(`mandate_execution::uniswap::evaluate_swap`) consumes. **This is fixture
data — no live Uniswap Trading API call is made.**

The sentinel host `sandbox.uniswap.invalid` (RFC 6761 §6.4 reserved
TLD) is used so the fixture cannot be mistaken for a live URL.

## What it demonstrates

Three numbered quotes with explicit per-quote expectations a swap-policy
author must satisfy:

1. **`happy_path_within_caps`** — bounded USDC → ETH, recipient on the
   treasury allowlist, slippage 35 bps (cap 50). Both `evaluate_swap`
   and the Mandate boundary should `Allow`.
2. **`multiple_violations_rug_quote`** — USDC → RUG with 1500 bps
   slippage and an off-allowlist recipient (`0x9999…`). This quote
   intentionally trips **three** swap-policy checks at once. `evaluate_swap`
   (`crates/mandate-execution/src/uniswap.rs`) traverses checks in
   field order — `input_token` → `output_token` → `max_notional_usd` →
   `max_slippage_bps` → `quote_freshness` → `treasury_recipient` — so
   the **first** violation a consumer sees is `output_token_allowlisted`
   (RUG ∉ output allowlist), with `max_slippage_bps` and
   `treasury_recipient_allowlisted` reported as additional violations
   in the same returned `SwapPolicyOutcome.checks` vector. The fixture
   reflects this with `expected_swap_policy_reason:
   "output_token_allowlisted"` plus
   `expected_additional_violations: ["max_slippage_bps",
   "treasury_recipient_allowlisted"]`. Mandate's policy boundary
   independently denies on `policy.deny_recipient_not_allowlisted`, so
   the deny is exercised at two layers (swap-policy guard + Mandate
   boundary) — defense in depth.
3. **`recipient_allowlist_violation`** — bounded slippage but recipient
   off-allowlist (the **only** failing check). `evaluate_swap` denies
   on `treasury_recipient_allowlisted`; Mandate denies on
   `policy.deny_recipient_not_allowlisted`. Demonstrates that the
   swap-policy guard and the Mandate policy boundary are **independent**
   defenses with different reason codes — defense in depth.

Each quote also carries an `expected_swap_policy` /
`expected_swap_policy_reason` (singular — the **first** failed check
under field-order traversal) / `expected_mandate_decision` /
`expected_mandate_deny_code` quadruple so an adapter author can dry-run
their policy against the fixture without guessing what the right answer
is. The rug entry adds `expected_additional_violations` (an array)
because that quote alone trips multiple checks.

## What live system it stands in for

The Uniswap Trading API quote endpoint. `UniswapExecutor::live()` in
`crates/mandate-execution/src/uniswap.rs` is intentionally stubbed
(`BackendOffline`) until that wiring lands; today the demo always
constructs `UniswapExecutor::local_mock()` and prints `mock: true`.

The pre-existing per-quote fixtures
[`uniswap/quote-USDC-ETH.json`](uniswap/quote-USDC-ETH.json) and
[`uniswap/quote-USDC-RUG.json`](uniswap/quote-USDC-RUG.json) are the
runtime inputs to gate 9 of the 13-gate demo; this catalogue is the
sponsor-reviewer view that puts all three deny / allow shapes into one
file.

## Exact replacement step

1. Implement `UniswapLiveConfig::from_env()` in
   `crates/mandate-execution/src/uniswap.rs` reading:
   - `MANDATE_UNISWAP_API_URL` — the Trading API quote endpoint.
   - `MANDATE_UNISWAP_API_KEY` — the API key (per-route quote).
   - `MANDATE_UNISWAP_CHAIN` — `mainnet` | `base` | `arbitrum` | etc.
2. Replace `UniswapExecutor::live()`'s `BackendOffline` stub with a real
   HTTP GET against the configured endpoint.
3. Wire `MANDATE_UNISWAP_LIVE=1` env-var gating into
   `demo-scripts/sponsors/uniswap-guarded-swap.sh` analogous to the
   `MANDATE_KEEPERHUB_LIVE=1` flag pattern (see
   [`docs/production-transition-checklist.md` §KeeperHub](../docs/production-transition-checklist.md#keeperhub-guarded-execution)
   for the env-var gating shape).
4. Address the "Suggested improvements" in
   [`FEEDBACK.md` §Uniswap](../FEEDBACK.md) before claiming live:
   - **signed quotes** with server-issued `quote_id` + `expires_at`
     anchored cryptographically into the Mandate decision token
   - **route token enumeration** so the per-path swap-policy guard
     can opt in/out at the path level, not just on input/output
   - **first-class slippage caps in the request** so the API can
     reject an over-slippage quote before Mandate ever sees it
5. The swap-policy guard (`evaluate_swap`) stays unchanged — it already
   runs against the canonical quote shape regardless of source.

See
[`docs/production-transition-checklist.md` §Uniswap](../docs/production-transition-checklist.md#uniswap-guarded-swap)
for the env-var / endpoint / credentials matrix.

## Truthfulness invariants

- Every quote has explicit expected outcomes (`expected_swap_policy`,
  `expected_mandate_decision`, `expected_mandate_deny_code`).
- The treasury-recipient allowlist is a single deterministic address
  (`0x111…`); the rug-token / violation path uses `0x999…`.
- The sentinel hostname is `sandbox.uniswap.invalid`.
- The fixture's envelope is enforced by
  [`test_fixtures.py`](test_fixtures.py).

## Where this fixture is referenced

- [`README.md`](README.md) §B3 fixtures
- [`test_fixtures.py`](test_fixtures.py) (validator)
- [`../FEEDBACK.md` §Uniswap](../FEEDBACK.md) (signed quotes, route
  enumeration, first-class slippage caps)
- [`../docs/production-transition-checklist.md` §Uniswap](../docs/production-transition-checklist.md#uniswap-guarded-swap)
- Runtime-consumed mocks: [`uniswap/quote-USDC-ETH.json`](uniswap/quote-USDC-ETH.json),
  [`uniswap/quote-USDC-RUG.json`](uniswap/quote-USDC-RUG.json)
  (gate 9 of `demo-scripts/run-openagents-final.sh`)
