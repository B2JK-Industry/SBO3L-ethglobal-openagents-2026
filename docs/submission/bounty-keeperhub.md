# SBO3L → KeeperHub Best Use

> **Audience:** KeeperHub bounty judges + Luca's team.
> **Length:** ~500 words. Detailed technical narrative at [`docs/keeperhub-integration-paths.md`](../keeperhub-integration-paths.md).

## Hero claim

**KeeperHub executes. SBO3L proves the execution was authorised.** Two complementary layers in the same agent stack — the policy boundary and the execution substrate — designed from the start to compose without either side absorbing the other's responsibility.

## Why this bounty

KeeperHub is positioned as the *execution layer for AI agents onchain*. The integration with SBO3L is a thin adapter, not a rewrite: KeeperHub records *what was executed*, SBO3L records *why it was authorised*. SBO3L's `KeeperHubExecutor::live_from_env()` POSTs the IP-1 envelope to a real workflow webhook the moment a `decision: allow` is signed; the executionId KeeperHub returns is captured into the Passport capsule's `execution.live_evidence`. Denied actions never reach the sponsor. Allowed actions arrive with a signed envelope that lets any auditor cryptographically link the upstream policy decision to the eventual KeeperHub execution row — without trusting either side to correlate honestly.

## Technical depth

Five integration paths (IP-1..IP-5) catalogued in [`docs/keeperhub-integration-paths.md`](../keeperhub-integration-paths.md), each independently small and reviewable:

| # | Shape | Adoption cost on KH side |
|---|---|---|
| **IP-1** | `sbo3l_*` upstream-proof envelope fields on the workflow webhook | 4-5 optional string fields, echo on lookup |
| **IP-2** | Public submission/result envelope JSON Schema | One JSON Schema file under your docs |
| **IP-3** | `keeperhub.lookup_execution(execution_id)` MCP tool | One MCP tool definition + thin handler |
| **IP-4** | Standalone `sbo3l-keeperhub-adapter` Rust crate | Listing on your "integrations" page; crates.io publication target |
| **IP-5** | SBO3L Passport capsule URI on the execution row | One optional string column |

Stacking the shapes gives **end-to-end offline auditability** of every KeeperHub execution that flowed through SBO3L. `sbo3l-keeperhub-adapter` is published standalone on crates.io ([1.2.0 LIVE](https://crates.io/crates/sbo3l-keeperhub-adapter)), so any third-party adapter author can depend on it without pulling the rest of SBO3L.

Adapter has both `local_mock()` (CI-safe default — produces a deterministic `kh-<ULID>` `execution_ref`, prints `mock: true`) and `live_from_env()` (real `wfb_…` token + workflow id) constructors. Mock and live are first-class peers; mock is the CI default for determinism, live is exercised explicitly per smoke gate.

## Live verification (judges click these)

- **Real KH workflow:** `https://app.keeperhub.com/api/workflows/m4t4cnpmhv8qquce3bv3c/webhook` — POST with the IP-1 envelope returns a real KH-format `executionId` (e.g. `kh-172o77rxov7mhwvpssc3x`). Verified end-to-end during the submission window.
- **Adapter on crates.io:** https://crates.io/crates/sbo3l-keeperhub-adapter — `cargo add sbo3l-keeperhub-adapter@1.2.0`
- **MCP tool implementation:** [`crates/sbo3l-mcp/src/lib.rs`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/crates/sbo3l-mcp/src/lib.rs) — `sbo3l.audit_lookup(execution_id)` mirrors the proposed `keeperhub.lookup_execution` shape
- **5 framework adapters shipped** — same policy-guarded KH executor across LangChain (TS + Py), CrewAI, AutoGen, ElizaOS, Vercel AI SDK:
  - [`@sbo3l/langchain-keeperhub`](https://www.npmjs.com/package/@sbo3l/langchain-keeperhub) (npm) + [`sbo3l-langchain-keeperhub`](https://pypi.org/project/sbo3l-langchain-keeperhub/) (PyPI, post-Trusted-Publisher)
  - [`sbo3l-crewai-keeperhub`](https://pypi.org/project/sbo3l-crewai-keeperhub/) (PyPI, post-Trusted-Publisher) — 3-agent crew sharing one policy boundary
  - [`sbo3l-autogen-keeperhub`](https://pypi.org/project/sbo3l-autogen-keeperhub/) (PyPI, post-Trusted-Publisher) — 2-agent planner+executor conversation
  - [`@sbo3l/elizaos-keeperhub`](https://www.npmjs.com/package/@sbo3l/elizaos-keeperhub) (npm) — chat-turn action handler
  - [`@sbo3l/vercel-ai-keeperhub`](https://www.npmjs.com/package/@sbo3l/vercel-ai-keeperhub) (npm) — Edge-compatible `tool()` for `streamText`/`generateText`
  - All sit composably alongside Devendra's `langchain-keeperhub` (PyPI) + Bleyle's ElizaOS plugin — they ship execution wrappers, ours ship policy-guarded execution wrappers
- **15 KH improvement issues filed** across 3 rounds ([`KeeperHub/cli#47-#62`](https://github.com/KeeperHub/cli/issues?q=is%3Aissue+author%3AB2JK-Industry)) — see the [Builder Feedback bounty one-pager](bounty-keeperhub-builder-feedback.md) + [round-3 evidence doc](../proof/kh-builder-feedback-2026-05-03.md)
- **Long-form post:** [`docs/proof/blog-keeperhub-composability.md`](../proof/blog-keeperhub-composability.md) — 1700-word composability writeup also live at [`/learn/keeperhub-composability`](https://sbo3l.dev/learn/keeperhub-composability)

## Sponsor-specific value prop

A KeeperHub auditor today reading an execution row has no cryptographic link back to whoever authorised the action. With IP-1 alone, that link becomes one offline verification. With IP-1 + IP-5, that link becomes one HTTP fetch. Neither IP-1 nor IP-5 requires KeeperHub to absorb any SBO3L logic — both are optional fields that KeeperHub echoes on lookup. This is the lightest possible adoption path that turns "trust me" into "verify it."

If KeeperHub merges IP-1 + IP-5 (estimated ~1 week of engineering on KH's side based on the field shapes), every execution row gains a falsifiable upstream proof — at zero runtime cost to KeeperHub, zero schema break for existing consumers.
