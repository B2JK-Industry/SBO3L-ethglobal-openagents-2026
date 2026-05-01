# Builder Feedback

Notes for partner sponsors during the ETHGlobal Open Agents 2026 build of **SBO3L**. This file is required for some partner prizes (e.g. Uniswap API integration) and is offered to all selected partners.

> **SBO3L Passport context.** The asks below are organised around what the *SBO3L Passport* product target needs from each partner surface in order to compose into a single proof-carrying execution flow. SBO3L Passport is *target product framing* — the capsule schema and verifier are tracked as A-side work in [`docs/product/SBO3L_PASSPORT_BACKLOG.md`](docs/product/SBO3L_PASSPORT_BACKLOG.md) and are not yet on `main`. Partner-specific one-pagers separate "what is implemented today" from "what is target": [`docs/partner-onepagers/keeperhub.md`](docs/partner-onepagers/keeperhub.md), [`docs/partner-onepagers/ens.md`](docs/partner-onepagers/ens.md), [`docs/partner-onepagers/uniswap.md`](docs/partner-onepagers/uniswap.md). Source of truth: [`docs/product/SBO3L_PASSPORT_SOURCE_OF_TRUTH.md`](docs/product/SBO3L_PASSPORT_SOURCE_OF_TRUTH.md).

## KeeperHub

How SBO3L uses KeeperHub: *SBO3L decides, KeeperHub executes.* After SBO3L signs an `allow` policy receipt, the receipt and the underlying APRP are handed to `KeeperHubExecutor::execute()`. Denied receipts are refused before any sponsor call. The same signed receipt can be packaged into a verifiable audit bundle (`sbo3l audit export` / `sbo3l audit verify-bundle`) so downstream audits can re-derive what KeeperHub was asked to execute, what was approved, and which audit-chain position the decision occupies.

### What worked

The "execution layer for AI agents onchain" framing maps directly onto our `GuardedExecutor` trait. The integration is a thin adapter, not a rewrite. Audit trails on KeeperHub's side complement our hash-chained audit log; the audit bundle gives KeeperHub a portable proof of *why* an action was authorised that any third-party auditor can re-verify offline.

### What was unclear at build time

- **Public submission/result schema.** We could not find a stable public schema for an action submission/result envelope, so the hackathon adapter mocks execution. This is clearly disclosed in script output (`mock: true`). The live path is a separate Rust constructor (`KeeperHubExecutor::live()`); the demo always constructs `KeeperHubExecutor::local_mock()`. Switching is one constructor call once a stable submission schema and credentials are available — there is no env-var feature flag in this hackathon build.
- **Token-prefix naming.** From outside the docs, the `kh_*` vs `wfb_*` prefix split (KeeperHub-native API tokens vs workflow-webhook tokens) wasn't obvious. A short "which token does which thing" page in the public docs — with a worked example showing the exact header each token belongs in — would shave significant integration time off third-party adapters.
- **`executionId` lookup.** It wasn't obvious how to look up the post-submit status (or run logs) of a previously-returned `executionId`. A documented GET path (or MCP tool — see below) would let policy engines and operators reconcile their own audit trails against KeeperHub's.

### Suggested improvements

- **Publish a JSON schema for action submission.** Third-party policy engines can validate locally before submitting; mismatches surface at the policy boundary, not over the wire.
- **Documented `executionId` → status / run-log lookup.** Either as a documented GET endpoint or as an MCP tool. SBO3L would call this from the operator console to render execution status next to the audit bundle that authorised it.
- **First-class upstream policy/audit fields on submission.** Native, schema-blessed fields a caller can attach to the submission envelope so KeeperHub's audit trail can re-emit them on the result side and in run logs:
  - `sbo3l_request_hash` — JCS-canonical SHA-256 of the APRP (SBO3L's canonical request hash).
  - `sbo3l_policy_hash` — canonical hash of the policy that authorised the action.
  - `sbo3l_receipt_signature` — Ed25519 signature of the policy receipt (hex).
  - `sbo3l_audit_event_id` — ULID of the audit-chain event the decision occupies.
  - `sbo3l_passport_capsule_hash` — content hash of the SBO3L Passport capsule (target field; the capsule schema (PR #42) and emit/verify CLI (PR #44) are on `main`. Populated once a small capsule-hash helper is wired into `KeeperHubExecutor::live()`'s envelope construction — A-side work tracked at [`docs/product/SBO3L_PASSPORT_BACKLOG.md`](docs/product/SBO3L_PASSPORT_BACKLOG.md)).
- **Idempotency semantics on workflow webhooks.** SBO3L already enforces HTTP `Idempotency-Key` safe-retry on its own ingest (PR #23 + #29). Documenting which header / field KeeperHub honours for caller-supplied retry keys, and what KeeperHub does on duplicate delivery (cached response vs new execution), would let policy engines safely retry a webhook submit without double-spending an authorisation.
- **Webhook signing / callback authenticity.** When a workflow webhook fires back to a SBO3L-style consumer, a documented signature scheme (e.g. `X-KeeperHub-Signature: sha256=<hex>` over the raw body, with a documented secret-rotation path) lets the consumer trust the inbound result without a side-channel.
- **MCP tool: `keeperhub.lookup_execution(execution_id)`** — returns status + run-log pointer + the `sbo3l_*` fields above (echoed back). Lets a SBO3L operator (or any auditor) connect a KeeperHub execution row directly to the upstream SBO3L authorization proof without out-of-band correlation.
- **Optional webhook headers from KeeperHub → caller.** When a workflow webhook fires back to a SBO3L-style consumer, attach two optional headers so the consumer can verify the upstream proof in one network round trip:
  - `X-SBO3L-Receipt-Signature: <hex>`
  - `X-SBO3L-Policy-Hash: <hex>`
- **Why these matter (in one sentence).** Today an auditor reading a KeeperHub execution row has no cryptographic link back to the SBO3L decision that approved it; with the four `sbo3l_*` fields plus the two optional headers, an offline auditor can take a KeeperHub execution log line, a SBO3L audit bundle, and verify end-to-end that the executed action was the one SBO3L signed off on — without trusting either side to correlate honestly.

### KeeperHub live integration target

This is what the live path looks like with a stable schema + credentials. The live arm of the adapter is now **shipped and verified end-to-end against a real KeeperHub workflow during the submission window**; the demo default still constructs `KeeperHubExecutor::local_mock()` for CI determinism. Documented here so KeeperHub's team can match the wire shape we POSTed.

- **Current build:** demo defaults to `KeeperHubExecutor::local_mock()` (deterministic `kh-<ULID>` `execution_ref`, prints `mock: true`) for CI determinism. **`KeeperHubExecutor::live()` is wired and verified end-to-end against a real workflow webhook** during the submission window — it POSTs the IP-1 envelope (request hash + policy hash + receipt signature + audit event id) to `SBO3L_KEEPERHUB_WEBHOOK_URL` with bearer auth (`SBO3L_KEEPERHUB_TOKEN`, must be `wfb_*` prefix), parses the response for `executionId`, and surfaces it as the `execution_ref` on the SBO3L `ExecutionReceipt`. **No real KeeperHub tokens or webhook URLs are committed anywhere** — `git grep` for `kh_[A-Za-z0-9]{8,}` / `wfb_[A-Za-z0-9]{8,}` / `KEEPERHUB_TOKEN` returns no real-token matches under `crates/`, `demo-scripts/`, `demo-fixtures/`, or `test-corpus/`; the only matches are prefix-validation literals (e.g. the string `"wfb_"` used in the live() entrypoint to reject misconfigured `kh_` tokens up front).
- **Target live flow:**
  1. Operator sets `SBO3L_KEEPERHUB_WEBHOOK_URL` and `SBO3L_KEEPERHUB_TOKEN` (or equivalent) in the daemon's environment. The token never enters the repo.
  2. SBO3L evaluates the APRP and signs an `allow` `PolicyReceipt`. Denied receipts are refused upstream — KeeperHub is never called for a denied action.
  3. `KeeperHubExecutor::live()` POSTs the signed receipt + the canonical APRP body to the workflow webhook, attaching the four `sbo3l_*` fields above on the envelope (and, when KeeperHub publishes the optional response headers, expecting them on any signed callback).
  4. The adapter parses the workflow's response, captures `executionId`, and returns it as the `execution_ref` on the SBO3L `ExecutionReceipt`.
  5. `executionId` is recorded in the SBO3L audit bundle (`sbo3l audit export` already carries `execution_ref` opaquely). The operator console renders it next to the corresponding audit-bundle verification panel.
  6. If the workflow returns a non-2xx status or an unparseable body, the live path surfaces an explicit `ExecutionError` — never a silent fallback to mock.
- **Truthfulness:** until the live path is exercised against a real KeeperHub workflow webhook, the demo continues to ship `local_mock()` and the demo runner continues to print `mock: true`. We will not flip the public surface to "live" without a real network call to a real KeeperHub endpoint.

## ENS

How SBO3L uses ENS: ENS is the agent's public identity. `research-agent.team.eth` resolves to `sbo3l:agent_id`, `sbo3l:endpoint`, `sbo3l:policy_hash`, `sbo3l:audit_root` and `sbo3l:proof_uri`. The demo verifies that the published policy hash matches the **active** SBO3L policy hash; drift is treated as un-trustable.

- **What worked:** text records are perfect for arbitrary structured metadata — no custom contract needed. The "policy hash matches what is published" pattern is a one-line check that gives judges immediate confidence.
- **What was unclear:** there is no canonical reverse pointer from a SBO3L-style identity back to its ENS name. The text-record namespace is a soft convention; we picked the `sbo3l:*` prefix and would happily move under a blessed `agent:*` namespace if the ecosystem standardises one.
- **Suggested improvements:**
  - A blessed text-record namespace for autonomous agents to prevent fragmentation. Today the `sbo3l:*` prefix is a soft convention; a standardised `agent:*` (or similar) namespace would let tooling agree without ad-hoc keys.
  - A canonical record for **policy commitment** so multiple security tools can share a slot rather than each picking their own key. SBO3L would publish its active policy hash there.
  - A canonical record for **proof URI** — a standardised slot for "where the public proof / capsule for this agent lives", so any client can find the proof without out-of-band convention. (For SBO3L this corresponds to the future `sbo3l.passport_capsule.v1` artefact published at the `sbo3l:proof_uri` value, target — see [`docs/partner-onepagers/ens.md`](docs/partner-onepagers/ens.md).)
  - **Endpoint-record guidance for agents.** Where the MCP/API endpoint a third-party tool should call to talk to the agent's policy gateway should live (alongside `url`, in a sub-namespace, etc.) is not standardised. The shipped offline ENS fixture currently uses `sbo3l:endpoint`; the Passport target introduces a more specific `sbo3l:mcp_endpoint` (or future blessed equivalent) once the MCP surface lands.

## Uniswap (stretch)

How SBO3L uses Uniswap: SBO3L sits in front of any agent that wants to swap. The flow is:
  1. The agent emits an APRP `smart_account_session` swap intent (input token, output token, max slippage bps, max notional USD, recipient).
  2. The Uniswap swap-policy guard (`sbo3l_execution::uniswap::evaluate_swap`) enforces input/output token allowlists, max notional, max slippage, quote freshness window and treasury-recipient guard.
  3. The APRP is posted to SBO3L's policy engine (under a swap-aware variant of the reference policy in `demo-fixtures/uniswap/sbo3l-policy.json`). SBO3L signs an `allow` receipt; denied swaps die at the policy boundary.
  4. Approved receipts are routed to `UniswapExecutor::local_mock()` which returns a deterministic `uni-<ULID>` execution_ref.

- **What worked:** the quote shape (input/output amount, route, slippage) maps cleanly onto a SBO3L `smart_account_session` APRP. The split between swap-policy guard (numeric/policy checks) and SBO3L's recipient/provider/budget boundary is natural.
- **What was hard:**
  - Quote freshness is implicit. We approximate freshness from local timestamps; the demo's static fixture has to relax this and we surface a `(relaxed)` flag explicitly so judges see it. Server-issued `quote_id` + `expires_at` would let policy engines anchor cryptographically into the receipt.
  - Multi-hop routes occasionally touch tokens that are not on our allowlist. We treat that as deny by default; explicit per-path token enumeration in the API response would let policy engines opt in or out at the path level.
- **Suggested improvements:**
  - **Signed quotes** — server-signed `quote_id + expires_at + canonical hash`. We would embed the canonical hash into the SBO3L decision token (and, target, into the `sbo3l.passport_capsule.v1` Uniswap evidence section — see [`docs/partner-onepagers/uniswap.md`](docs/partner-onepagers/uniswap.md)) so the on-chain executor can require the same quote that authorised the action.
  - **`expires_at` on the quote response.** Today freshness is approximated from local timestamps; the demo's static fixture has to relax this and we surface a `(relaxed)` flag explicitly so judges see it. A server-side `expires_at` removes the approximation entirely.
  - **Route token enumeration** — list every intermediate token, not just input/output, so per-path swap-policy guards can opt in or out of multi-hop routes at the path level instead of denying by default.
  - **First-class slippage caps in the request** — letting the API reject a quote whose realised slippage already exceeds the caller's cap removes a class of agent footguns at the network boundary, before SBO3L's `evaluate_swap` ever sees the quote.
  - **Slippage-cap semantics, documented.** Whether `slippageBps` in the request is "max acceptable realised slippage" vs "request the route to target this slippage" is not obvious from the integration guide — third-party policy engines need that distinction explicit, with a worked example.
  - **Canonical quote hash.** A documented JCS-shape (or equivalent) over the quote response so third-party policy engines can hash deterministically without inventing a canonicalisation. SBO3L already canonicalises APRP via JCS for `request_hash`; a server-side canonical quote hash slots into the same pattern.

### Friction we hit while wiring the P6.1 quote-evidence capsule

These are concrete things that slowed us down while building
`UniswapQuoteEvidence` against the demo's mock quote shape and
projecting it into the Passport capsule's `execution.executor_evidence`
slot. They are not blockers, but each of them would have shaved real
implementation time off if the public Trading API surface answered them
upstream:

- **Freshness window is a guess.** `UniswapQuoteEvidence` ships with
  `quote_freshness_seconds: 30` because that is conservative enough for
  every single test fixture we have, but the *correct* value should
  come from the quote response. Today we have to pick one number, hard-
  code it, and surface a `(relaxed)` flag on the demo runner so judges
  can see we relaxed it. A server-side `expires_at` (or
  `freshness_horizon_seconds`) on the quote response — ideally on every
  quote, not just paid plans — would let us drop the hard-coded
  constant entirely.
- **V2/V3 router-address split is invisible to the policy layer.** The
  treasury-recipient allowlist is the same regardless of which router
  contract is hit, but the *quote response* doesn't tell us which
  router will execute. This is fine today (we're not on-chain), but
  any policy engine that wants to maintain different allowlists for V2
  vs V3 (or for upcoming Uniswap V4 hooks) would need that field to
  appear in the canonical quote. We left the field unset in
  `UniswapQuoteEvidence` rather than guess.
- **Multi-hop route token enumeration is silent.** Our `route_tokens`
  array currently echoes back input → output for the demo's direct
  routes, but a multi-hop quote's intermediate tokens (e.g. a USDC →
  WBTC → ETH path) aren't in the response shape we pinned. We'd want
  every intermediate token spelled out so the swap-policy guard can
  reject a route that touches a token the policy doesn't allow. Today
  we treat that as deny-by-default; explicit per-path token enumeration
  in the API response would let policy engines opt in or out at the
  path level.
- **Slippage-cap semantics ambiguity (still).** `slippage_cap_bps` in
  `UniswapQuoteEvidence` is "max acceptable realised slippage on
  execution" — the same cap the swap-policy guard checks. But when we
  project this into a hypothetical live request, the spec is unclear
  whether `slippageBps` in the request is "max acceptable" vs "target".
  We left the evidence field with the strict-max semantics; we'd value
  a documented `slippage_cap_semantics: "max_realised" | "target"`
  field on the canonical quote response so third-party policy engines
  don't have to pick one and hope.
- **No canonical hash of the quote.** The capsule's
  `executor_evidence` is byte-stable for the demo, but a third-party
  re-verifier today has to canonicalise the quote on their side
  (JCS-shape) to compare. A documented server-side canonical quote
  hash (matching the JCS shape SBO3L already uses for `request_hash`)
  would let us anchor the hash directly into the SBO3L decision
  token without inventing a canonicalisation.

### Known limitations of the hackathon implementation

- `demo-scripts/sponsors/uniswap-guarded-swap.sh` runs against a stored quote fixture by default. The shipped live path is `UniswapExecutor::live_from_env()` (env-gated on `SBO3L_UNISWAP_RPC_URL` + `SBO3L_UNISWAP_TOKEN_OUT`), which hits Sepolia QuoterV2 (`0xEd1f6473345F45b75F8179591dd5bA1888cf2FB3`) for a real read-side `quoteExactInputSingle` call — verified end-to-end against Sepolia during the submission window. The bare back-compat `UniswapExecutor::live()` ctor returns `BackendOffline` at runtime. Real swap broadcast (Trading API) remains scope-cut — only the read-side quote evidence is wired today.
- The treasury recipient check uses the demo allowlist; production deployments should source it from the active SBO3L policy's `recipients` list (already supported by `sbo3l-policy::Policy`).
- `UniswapQuoteEvidence::quote_source` is hard-coded to the string `"mock-uniswap-v3-router"` and the `quote_id` carries a `mock-` prefix on the demo path — explicit honest-disclosure so judges (and any auditor reading a capsule offline) cannot mistake the demo evidence for a real Trading API response. When the live path lands, both fields flip to the real router endpoint URL and the real server-issued quote id.

## General

- The agent-identity → policy-hash → audit-root pattern via ENS text records felt natural; partners that resolve ENS metadata should consider standardising the `sbo3l:*` keys.
- Sponsor adapters benefit from a clean separation between "decide" (policy) and "execute" (sponsor). This is the architectural angle SBO3L wants to validate.

## Concrete pain points hit during live integration

The five highest-friction items encountered during the live SBO3L ↔ partner wiring, distilled from the per-partner sections above. Each is filed as a GitHub issue against the upstream partner repo so the discussion can carry on past the hackathon submission window.

- **KeeperHub — undocumented submission/result schema.** No public JSON schema for the action submission/result envelope; the hackathon adapter mocks execution. See *KeeperHub → What was unclear at build time* above.
- **KeeperHub — `executionId` lookup undocumented.** No documented GET path or MCP tool for post-submit status / run-log retrieval. See *KeeperHub → Suggested improvements*.
- **KeeperHub — token-prefix naming (`kh_*` vs `wfb_*`).** The split between native API tokens and workflow-webhook tokens isn't surfaced in the public docs; cost real wiring time. See *KeeperHub → What was unclear at build time*.
- **ENS — ENSIP-25 CCIP-Read off-chain extension.** The reference implementation lacks an end-to-end "registrar + resolver + gateway + capsule" worked example for AI-agent identities; we ended up reading the spec + ENS Labs OffchainResolver source to wire it correctly. See *ENS* section.
- **Uniswap — quote evidence capsule attribution.** The Sepolia QuoterV2 returns the quote tuple but no server-issued `quote_id`, so we mint our own `mock-` prefixed id on the demo path. See *Uniswap → Friction we hit while wiring the P6.1 quote-evidence capsule*.
