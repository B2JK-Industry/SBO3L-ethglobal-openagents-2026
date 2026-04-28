# Builder Feedback

Notes for partner sponsors during the ETHGlobal Open Agents 2026 build of **Mandate**. This file is required for some partner prizes (e.g. Uniswap API integration) and is offered to all selected partners.

## KeeperHub

How Mandate uses KeeperHub: *Mandate decides, KeeperHub executes.* After Mandate signs an `allow` policy receipt, the receipt and the underlying APRP are handed to `KeeperHubExecutor::execute()`. Denied receipts are refused before any sponsor call.

- **What worked:** the "execution layer for AI agents onchain" framing maps directly onto our `GuardedExecutor` trait. The integration is a thin adapter, not a rewrite. Audit trails on KeeperHub's side complement our hash-chained audit log.
- **What was unclear:** at build time we could not find a stable public schema for an action submission/result envelope, so the hackathon adapter mocks execution. This is clearly disclosed in script output (`mock: true`). The live path is a separate Rust constructor (`KeeperHubExecutor::live()`); the demo always constructs `KeeperHubExecutor::local_mock()`. Switching is one constructor call once a stable submission schema and credentials are available — there is no env-var feature flag in this hackathon build.
- **Suggested improvements:**
  - Publish a JSON schema for action submission so third-party policy engines can validate locally before submitting.
  - Native field for an upstream policy/receipt id so KeeperHub's audit trail can re-emit it and tie executions back to whoever authorised them.

## ENS

How Mandate uses ENS: ENS is the agent's public identity. `research-agent.team.eth` resolves to `mandate:agent_id`, `mandate:endpoint`, `mandate:policy_hash`, `mandate:audit_root` and `mandate:receipt_schema`. The demo verifies that the published policy hash matches the **active** Mandate policy hash; drift is treated as un-trustable.

- **What worked:** text records are perfect for arbitrary structured metadata — no custom contract needed. The "policy hash matches what is published" pattern is a one-line check that gives judges immediate confidence.
- **What was unclear:** there is no canonical reverse pointer from a Mandate-style identity back to its ENS name. The text-record namespace is a soft convention; we picked the `mandate:*` prefix and would happily move under a blessed `agent:*` namespace if the ecosystem standardises one.
- **Suggested improvements:**
  - A blessed text-record namespace for autonomous agents to prevent fragmentation.
  - A canonical record for "policy commitment" so multiple security tools can share a slot rather than each picking their own key.

## Uniswap (stretch)

How Mandate uses Uniswap: Mandate sits in front of any agent that wants to swap. The flow is:
  1. The agent emits an APRP `smart_account_session` swap intent (input token, output token, max slippage bps, max notional USD, recipient).
  2. The Uniswap swap-policy guard (`mandate_execution::uniswap::evaluate_swap`) enforces input/output token allowlists, max notional, max slippage, quote freshness window and treasury-recipient guard.
  3. The APRP is posted to Mandate's policy engine (under a swap-aware variant of the reference policy in `demo-fixtures/uniswap/mandate-policy.json`). Mandate signs an `allow` receipt; denied swaps die at the policy boundary.
  4. Approved receipts are routed to `UniswapExecutor::local_mock()` which returns a deterministic `uni-<ULID>` execution_ref.

- **What worked:** the quote shape (input/output amount, route, slippage) maps cleanly onto a Mandate `smart_account_session` APRP. The split between swap-policy guard (numeric/policy checks) and Mandate's recipient/provider/budget boundary is natural.
- **What was hard:**
  - Quote freshness is implicit. We approximate freshness from local timestamps; the demo's static fixture has to relax this and we surface a `(relaxed)` flag explicitly so judges see it. Server-issued `quote_id` + `expires_at` would let policy engines anchor cryptographically into the receipt.
  - Multi-hop routes occasionally touch tokens that are not on our allowlist. We treat that as deny by default; explicit per-path token enumeration in the API response would let policy engines opt in or out at the path level.
- **Suggested improvements:**
  - **Signed quotes** — server-signed `quote_id + expires_at + canonical hash`. We would embed the hash into the Mandate decision token so the on-chain executor can require the same quote.
  - **Route token enumeration** — list every intermediate token, not just input/output.
  - **First-class slippage caps in the request** — letting the API reject a quote whose realised slippage already exceeds the caller's cap removes a class of agent footguns.

### Known limitations of the hackathon implementation

- `demo-scripts/sponsors/uniswap-guarded-swap.sh` runs against a stored quote fixture. The live path (`UniswapExecutor::live()`) is intentionally stubbed in this hackathon build and would error with `BackendOffline`; the demo always uses `UniswapExecutor::local_mock()`. There is no env-var feature flag in this build — wiring up a real Uniswap Trading API endpoint is one function-body change.
- The treasury recipient check uses the demo allowlist; production deployments should source it from the active Mandate policy's `recipients` list (already supported by `mandate-policy::Policy`).

## General

- The agent-identity → policy-hash → audit-root pattern via ENS text records felt natural; partners that resolve ENS metadata should consider standardising the `mandate:*` keys.
- Sponsor adapters benefit from a clean separation between "decide" (policy) and "execute" (sponsor). This is the architectural angle Mandate wants to validate.
