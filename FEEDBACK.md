# Builder Feedback

Notes for partner sponsors during the ETHGlobal Open Agents 2026 build of **Mandate**. This file is required for some partner prizes (e.g. Uniswap API integration) and is offered to all selected partners.

> **Mandate Passport context.** The asks below are organised around what the *Mandate Passport* product target needs from each partner surface in order to compose into a single proof-carrying execution flow. Mandate Passport is *target product framing* — the capsule schema and verifier are tracked as A-side work in [`docs/product/MANDATE_PASSPORT_BACKLOG.md`](docs/product/MANDATE_PASSPORT_BACKLOG.md) and are not yet on `main`. Partner-specific one-pagers separate "what is implemented today" from "what is target": [`docs/partner-onepagers/keeperhub.md`](docs/partner-onepagers/keeperhub.md), [`docs/partner-onepagers/ens.md`](docs/partner-onepagers/ens.md), [`docs/partner-onepagers/uniswap.md`](docs/partner-onepagers/uniswap.md). Source of truth: [`docs/product/MANDATE_PASSPORT_SOURCE_OF_TRUTH.md`](docs/product/MANDATE_PASSPORT_SOURCE_OF_TRUTH.md).

## KeeperHub

How Mandate uses KeeperHub: *Mandate decides, KeeperHub executes.* After Mandate signs an `allow` policy receipt, the receipt and the underlying APRP are handed to `KeeperHubExecutor::execute()`. Denied receipts are refused before any sponsor call. The same signed receipt can be packaged into a verifiable audit bundle (`mandate audit export` / `mandate audit verify-bundle`) so downstream audits can re-derive what KeeperHub was asked to execute, what was approved, and which audit-chain position the decision occupies.

### What worked

The "execution layer for AI agents onchain" framing maps directly onto our `GuardedExecutor` trait. The integration is a thin adapter, not a rewrite. Audit trails on KeeperHub's side complement our hash-chained audit log; the audit bundle gives KeeperHub a portable proof of *why* an action was authorised that any third-party auditor can re-verify offline.

### What was unclear at build time

- **Public submission/result schema.** We could not find a stable public schema for an action submission/result envelope, so the hackathon adapter mocks execution. This is clearly disclosed in script output (`mock: true`). The live path is a separate Rust constructor (`KeeperHubExecutor::live()`); the demo always constructs `KeeperHubExecutor::local_mock()`. Switching is one constructor call once a stable submission schema and credentials are available — there is no env-var feature flag in this hackathon build.
- **Token-prefix naming.** From outside the docs, the `kh_*` vs `wfb_*` prefix split (KeeperHub-native API tokens vs workflow-webhook tokens) wasn't obvious. A short "which token does which thing" page in the public docs — with a worked example showing the exact header each token belongs in — would shave significant integration time off third-party adapters.
- **`executionId` lookup.** It wasn't obvious how to look up the post-submit status (or run logs) of a previously-returned `executionId`. A documented GET path (or MCP tool — see below) would let policy engines and operators reconcile their own audit trails against KeeperHub's.

### Suggested improvements

- **Publish a JSON schema for action submission.** Third-party policy engines can validate locally before submitting; mismatches surface at the policy boundary, not over the wire.
- **Documented `executionId` → status / run-log lookup.** Either as a documented GET endpoint or as an MCP tool. Mandate would call this from the operator console to render execution status next to the audit bundle that authorised it.
- **First-class upstream policy/audit fields on submission.** Native, schema-blessed fields a caller can attach to the submission envelope so KeeperHub's audit trail can re-emit them on the result side and in run logs:
  - `mandate_request_hash` — JCS-canonical SHA-256 of the APRP (Mandate's canonical request hash).
  - `mandate_policy_hash` — canonical hash of the policy that authorised the action.
  - `mandate_receipt_signature` — Ed25519 signature of the policy receipt (hex).
  - `mandate_audit_event_id` — ULID of the audit-chain event the decision occupies.
  - `mandate_passport_capsule_hash` — content hash of the Mandate Passport capsule (target field; populated once the Passport capsule schema lands on `main` — A-side work tracked at [`docs/product/MANDATE_PASSPORT_BACKLOG.md` §P1.1](docs/product/MANDATE_PASSPORT_BACKLOG.md)).
- **Idempotency semantics on workflow webhooks.** Mandate already enforces HTTP `Idempotency-Key` safe-retry on its own ingest (PR #23 + #29). Documenting which header / field KeeperHub honours for caller-supplied retry keys, and what KeeperHub does on duplicate delivery (cached response vs new execution), would let policy engines safely retry a webhook submit without double-spending an authorisation.
- **Webhook signing / callback authenticity.** When a workflow webhook fires back to a Mandate-style consumer, a documented signature scheme (e.g. `X-KeeperHub-Signature: sha256=<hex>` over the raw body, with a documented secret-rotation path) lets the consumer trust the inbound result without a side-channel.
- **MCP tool: `keeperhub.lookup_execution(execution_id)`** — returns status + run-log pointer + the `mandate_*` fields above (echoed back). Lets a Mandate operator (or any auditor) connect a KeeperHub execution row directly to the upstream Mandate authorization proof without out-of-band correlation.
- **Optional webhook headers from KeeperHub → caller.** When a workflow webhook fires back to a Mandate-style consumer, attach two optional headers so the consumer can verify the upstream proof in one network round trip:
  - `X-Mandate-Receipt-Signature: <hex>`
  - `X-Mandate-Policy-Hash: <hex>`
- **Why these matter (in one sentence).** Today an auditor reading a KeeperHub execution row has no cryptographic link back to the Mandate decision that approved it; with the four `mandate_*` fields plus the two optional headers, an offline auditor can take a KeeperHub execution log line, a Mandate audit bundle, and verify end-to-end that the executed action was the one Mandate signed off on — without trusting either side to correlate honestly.

### KeeperHub live integration target

This is what the live path looks like once a stable schema + credentials are available. It is **not implemented** in this hackathon build (which honestly mocks execution behind `KeeperHubExecutor::local_mock()`); it is documented here so KeeperHub's team can pre-empt the obvious questions during review.

- **Current build:** `KeeperHubExecutor::local_mock()` returns a deterministic `kh-<ULID>` `execution_ref` and prints `mock: true` in demo output. `KeeperHubExecutor::live()` is a separate Rust constructor and is the only place the live wiring needs to land. **No KeeperHub credentials, secrets, tokens, or fixtures are committed anywhere in this repo** — `git grep` for `kh_`, `wfb_`, `KEEPERHUB_TOKEN`, `KEEPERHUB_API_KEY` returns nothing under `crates/`, `demo-scripts/`, `demo-fixtures/`, or `test-corpus/`.
- **Target live flow:**
  1. Operator sets `MANDATE_KEEPERHUB_WEBHOOK_URL` and `MANDATE_KEEPERHUB_TOKEN` (or equivalent) in the daemon's environment. The token never enters the repo.
  2. Mandate evaluates the APRP and signs an `allow` `PolicyReceipt`. Denied receipts are refused upstream — KeeperHub is never called for a denied action.
  3. `KeeperHubExecutor::live()` POSTs the signed receipt + the canonical APRP body to the workflow webhook, attaching the four `mandate_*` fields above on the envelope (and, when KeeperHub publishes the optional response headers, expecting them on any signed callback).
  4. The adapter parses the workflow's response, captures `executionId`, and returns it as the `execution_ref` on the Mandate `ExecutionReceipt`.
  5. `executionId` is recorded in the Mandate audit bundle (`mandate audit export` already carries `execution_ref` opaquely). The operator console renders it next to the corresponding audit-bundle verification panel.
  6. If the workflow returns a non-2xx status or an unparseable body, the live path surfaces an explicit `ExecutionError` — never a silent fallback to mock.
- **Truthfulness:** until the live path is exercised against a real KeeperHub workflow webhook, the demo continues to ship `local_mock()` and the demo runner continues to print `mock: true`. We will not flip the public surface to "live" without a real network call to a real KeeperHub endpoint.

## ENS

How Mandate uses ENS: ENS is the agent's public identity. `research-agent.team.eth` resolves to `mandate:agent_id`, `mandate:endpoint`, `mandate:policy_hash`, `mandate:audit_root` and `mandate:receipt_schema`. The demo verifies that the published policy hash matches the **active** Mandate policy hash; drift is treated as un-trustable.

- **What worked:** text records are perfect for arbitrary structured metadata — no custom contract needed. The "policy hash matches what is published" pattern is a one-line check that gives judges immediate confidence.
- **What was unclear:** there is no canonical reverse pointer from a Mandate-style identity back to its ENS name. The text-record namespace is a soft convention; we picked the `mandate:*` prefix and would happily move under a blessed `agent:*` namespace if the ecosystem standardises one.
- **Suggested improvements:**
  - A blessed text-record namespace for autonomous agents to prevent fragmentation. Today the `mandate:*` prefix is a soft convention; a standardised `agent:*` (or similar) namespace would let tooling agree without ad-hoc keys.
  - A canonical record for **policy commitment** so multiple security tools can share a slot rather than each picking their own key. Mandate would publish its active policy hash there.
  - A canonical record for **proof URI** — a standardised slot for "where the public proof / capsule for this agent lives", so any client can find the proof without out-of-band convention. (For Mandate this corresponds to the future `mandate.passport_capsule.v1` artefact published at the `mandate:proof_uri` value, target — see [`docs/partner-onepagers/ens.md`](docs/partner-onepagers/ens.md).)
  - **Endpoint-record guidance for agents.** Where the MCP/API endpoint a third-party tool should call to talk to the agent's policy gateway should live (alongside `url`, in a sub-namespace, etc.) is not standardised; today we self-publish under `mandate:mcp_endpoint`.

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
  - **Signed quotes** — server-signed `quote_id + expires_at + canonical hash`. We would embed the canonical hash into the Mandate decision token (and, target, into the `mandate.passport_capsule.v1` Uniswap evidence section — see [`docs/partner-onepagers/uniswap.md`](docs/partner-onepagers/uniswap.md)) so the on-chain executor can require the same quote that authorised the action.
  - **`expires_at` on the quote response.** Today freshness is approximated from local timestamps; the demo's static fixture has to relax this and we surface a `(relaxed)` flag explicitly so judges see it. A server-side `expires_at` removes the approximation entirely.
  - **Route token enumeration** — list every intermediate token, not just input/output, so per-path swap-policy guards can opt in or out of multi-hop routes at the path level instead of denying by default.
  - **First-class slippage caps in the request** — letting the API reject a quote whose realised slippage already exceeds the caller's cap removes a class of agent footguns at the network boundary, before Mandate's `evaluate_swap` ever sees the quote.
  - **Slippage-cap semantics, documented.** Whether `slippageBps` in the request is "max acceptable realised slippage" vs "request the route to target this slippage" is not obvious from the integration guide — third-party policy engines need that distinction explicit, with a worked example.
  - **Canonical quote hash.** A documented JCS-shape (or equivalent) over the quote response so third-party policy engines can hash deterministically without inventing a canonicalisation. Mandate already canonicalises APRP via JCS for `request_hash`; a server-side canonical quote hash slots into the same pattern.

### Known limitations of the hackathon implementation

- `demo-scripts/sponsors/uniswap-guarded-swap.sh` runs against a stored quote fixture. The live path (`UniswapExecutor::live()`) is intentionally stubbed in this hackathon build and would error with `BackendOffline`; the demo always uses `UniswapExecutor::local_mock()`. There is no env-var feature flag in this build — wiring up a real Uniswap Trading API endpoint is one function-body change.
- The treasury recipient check uses the demo allowlist; production deployments should source it from the active Mandate policy's `recipients` list (already supported by `mandate-policy::Policy`).

## General

- The agent-identity → policy-hash → audit-root pattern via ENS text records felt natural; partners that resolve ENS metadata should consider standardising the `mandate:*` keys.
- Sponsor adapters benefit from a clean separation between "decide" (policy) and "execute" (sponsor). This is the architectural angle Mandate wants to validate.
