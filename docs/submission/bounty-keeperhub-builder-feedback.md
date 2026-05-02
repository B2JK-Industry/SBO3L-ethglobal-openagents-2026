# SBO3L → KeeperHub Builder Feedback

> **Audience:** KeeperHub bounty judges + KH platform/docs team.
> **Length:** ~500 words.

## Hero claim

**Five concrete asks filed during real integration work**, every one motivated by a specific friction point we hit while building [`sbo3l-keeperhub-adapter`](https://crates.io/crates/sbo3l-keeperhub-adapter) and the live arm of `KeeperHubExecutor::live_from_env()`.

## Why this bounty

The Builder Feedback bounty rewards *specific, actionable* feedback from teams that actually shipped against KeeperHub, not generic "would be nice" suggestions. Every issue we filed includes a worked code example, a citation to the exact line in our adapter where the friction surfaced, and a proposed shape for the fix. We want the KeeperHub platform to win with us, not just be polite about reviewing our PR.

## Issues filed

| Issue | Subject | Friction we hit |
|---|---|---|
| [KeeperHub/cli#47](https://github.com/KeeperHub/cli/issues/47) | Token-prefix naming (`kh_*` vs `wfb_*`) | Cost real wiring time — split between native API tokens and workflow-webhook tokens isn't surfaced in the public docs. We filed a worked example showing the exact header each token belongs in. |
| [KeeperHub/cli#48](https://github.com/KeeperHub/cli/issues/48) | Undocumented submission/result envelope schema | No public JSON schema for the action submission/result envelope — our hackathon adapter mocks execution by default for CI determinism. Live submissions work, but the envelope shape was reverse-engineered from `curl -v`. Proposed a JSON Schema file to mirror our expectations. |
| [KeeperHub/cli#49](https://github.com/KeeperHub/cli/issues/49) | `executionId` lookup undocumented | No documented GET path or MCP tool for post-submit status / run-log retrieval. Policy engines (and operators) need to reconcile their own audit trails against KeeperHub's; today there's no canonical lookup. Proposed `keeperhub.lookup_execution(execution_id)` MCP tool shape. |
| [KeeperHub/cli#50](https://github.com/KeeperHub/cli/issues/50) | First-class upstream policy/audit fields on submission | Today an auditor reading a KH execution row has no cryptographic link back to the SBO3L decision that approved it. Proposed five optional `sbo3l_*` envelope fields KH can echo on lookup (request_hash, policy_hash, receipt_signature, audit_event_id, optional passport_uri). |
| [KeeperHub/cli#51](https://github.com/KeeperHub/cli/issues/51) | `Idempotency-Key` retry semantics on workflow webhooks | Undocumented dedup behavior on duplicate webhook delivery. Policy engines have to choose between "never retry" (loses delivery) or "retry blindly" (risks double-spending an authorisation). Proposed: KeeperHub honour `Idempotency-Key` header per-webhook with documented dedup window. |

All five issues are linked from [`FEEDBACK.md`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/FEEDBACK.md) — this is the canonical source SBO3L's Phase 1 exit gate `T-2-1` ([PR #172](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/172)) literal-greps against to confirm submission.

## What makes this feedback different

**It's all reproducible.** Every issue has either a `curl` invocation or a Rust unit test that demonstrates the friction. None of the asks are aesthetic — they're all about making integration deterministic for third-party adapter authors. We expect any one of them to land within KeeperHub's normal docs/feature flow without coordination, since each is independent.

**It's all from real integration work.** SBO3L's `KeeperHubExecutor::live_from_env()` was built end-to-end during the hackathon, against a real `wfb_…` token, against the real workflow `m4t4cnpmhv8qquce3bv3c`. The adapter is published standalone on crates.io (IP-4 path); anyone can `cargo add sbo3l-keeperhub-adapter@1.0.1` and reproduce the exact integration shape that surfaced these frictions.

## Sponsor-specific value prop

Builder Feedback is about *closing the loop* — teams that hit friction become the team that fixes the docs, designs the schema, or proposes the MCP tool. We want to ship IP-1..IP-5 with KH, not for SBO3L (we already shipped our side); we want it because the same friction will hit the next twenty teams that try this integration.

The fastest path to landing all five: KH merges IP-1 (5 optional fields) + IP-2 (one JSON Schema file) — that's a ~1-day platform-team task that unlocks every other adapter author after us.
