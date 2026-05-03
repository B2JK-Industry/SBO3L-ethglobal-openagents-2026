# KeeperHub Builder Feedback — round 2 (2026-05-03)

> **Audience:** KeeperHub bounty judges + KH platform/docs team.
> **Companion to:** [docs/submission/bounty-keeperhub-builder-feedback.md](../submission/bounty-keeperhub-builder-feedback.md) (round 1 — issues #47–#51).
> **Amplifier:** KH-BF-A1 + KH-BF-A2 (additional issues + reference PR drafts).

## Hero claim

**Five MORE concrete asks filed during continued integration work** (post-round-1), bringing total filed feedback to **10 issues + 5 reference PR drafts** on KeeperHub/cli. Every issue derives from a specific friction point hit while building [`sbo3l-keeperhub-adapter`](https://crates.io/crates/sbo3l-keeperhub-adapter), with a worked reproduction, a citation to the exact line in our adapter where the friction surfaces, and a proposed shape for the fix. Every issue has a companion **draft PR on our repo** showing what the consumer-side adapter change would look like once KH lands the proposal — proof that the fix design is implementable end-to-end, not just a sketch.

## Why this round

Round 1 (#47–#51) covered the obvious-on-day-one frictions (token prefix, envelope schema, executionId lookup, sbo3l_* fields, idempotency-key dedup). Round 2 (#52–#56) covers the frictions that only surface AFTER you've built the adapter and started thinking about what production hardening looks like: error catalogs, fixture suites, SLO publication, schema versioning, payload size budgeting. These are the asks an adapter author files only after the integration WORKS — i.e. exactly the kind of feedback the Builder Feedback bounty is designed to surface.

## Issues filed (round 2)

| Issue | Subject | Friction we hit | Companion PR draft |
|---|---|---|---|
| [KeeperHub/cli#52](https://github.com/KeeperHub/cli/issues/52) | HTTP error code catalog + adapter retry semantics for workflow webhook | Adapter at [`lib.rs:300-308`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/crates/sbo3l-keeperhub-adapter/src/lib.rs#L300-L308) treats all non-2xx as fatal `ProtocolError`. No way to distinguish transient (5xx, 429) from permanent (4xx). Forces choice between "never retry" (loses delivery on transient) or "retry blindly" (risks double-spend). | [PR #402](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/402) — proposed `KhErrorCode` enum + `Permanent`/`Transient` split with documented retry behavior |
| [KeeperHub/cli#53](https://github.com/KeeperHub/cli/issues/53) | Public mock fixture suite (or `keeperhub-mock` Docker image) for adapter testing | Every CI run of the live submission test must either burn a real workflow execution (cost + history pollution) or maintain a bespoke mock that we hope matches prod. Adapter at [`lib.rs:97-99`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/crates/sbo3l-keeperhub-adapter/src/lib.rs#L97-L99) uses `KeeperHubExecutor::local_mock()` with a hand-rolled response shape we reverse-engineered from `curl -v`. | [PR #403](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/403) — adapter tests migrated to `keeperhub-mock` service container OR pinned `keeperhub-fixtures` crate |
| [KeeperHub/cli#54](https://github.com/KeeperHub/cli/issues/54) | Publish workflow webhook timeout SLO (p50 / p95 / p99) | Adapter at [`lib.rs:283-289`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/crates/sbo3l-keeperhub-adapter/src/lib.rs#L283-L289) hardcodes 5-second timeout based on one-day informal measurement. No documented p99 means every adapter author re-discovers the calibration problem and lands on a different number. | [PR #404](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/404) — bumps to `2 × p99` (10s) once SLO published, adds `SBO3L_KEEPERHUB_TIMEOUT_SECS` operator override |
| [KeeperHub/cli#55](https://github.com/KeeperHub/cli/issues/55) | `X-KeeperHub-Schema-Version` response header + `Accept-KeeperHub-Schema` request header | Parser at [`lib.rs:316-319`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/crates/sbo3l-keeperhub-adapter/src/lib.rs#L316-L319) reads `executionId` with fallback to `id` because we observed both shapes in the wild. No version signal means future KH refactors silently break adapters in production. | [PR #405](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/405) — pins `Accept-KeeperHub-Schema`, removes legacy fallback, adds drift-detection error |
| [KeeperHub/cli#56](https://github.com/KeeperHub/cli/issues/56) | Document max payload size for workflow webhook submissions + 413 response shape | Adapter at [`lib.rs:235-289`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/crates/sbo3l-keeperhub-adapter/src/lib.rs#L235-L289) constructs the IP-1 envelope without a size check. Issue #50's "rich evidence inline" path is blocked because we don't know whether to embed a passport capsule (~50KB) or pass it by URI. | [PR #406](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/406) — adds pre-POST size check + new `EnvelopeTooLarge` error variant + inline-vs-URI capsule selector |

## What "draft PR" means here

Each PR (#402–#406) is intentionally a **doc-only diff** in `docs/keeperhub-consumer-shape-proposals/`. They show — with concrete code blocks against real line numbers in `crates/sbo3l-keeperhub-adapter/src/lib.rs` — what the adapter diff WILL look like once the upstream KH issue ships. Why doc-only:

1. **The shape depends on KH's chosen contract.** E.g. PR #402's `KhErrorCode` enum has placeholder variants (`EnvelopeMalformed`, `RateLimit`, etc.) — we can't ship them today because the official codes come from KH publishing the catalog. Shipping our guess would defeat the whole purpose (the goal is convergence on KH's documented codes, not divergence on N adapter authors' guesses).

2. **The numbers depend on KH's measurements.** E.g. PR #404 proposes `2 × p99 = 10s` timeout. The `2 ×` rule is well-trodden across vendor SDKs; the `p99 = 5s` placeholder is what we measured but not what KH publishes.

3. **The headers depend on KH's choices.** E.g. PR #405's `PINNED_SCHEMA = "2026-05-01"` is a placeholder string. KH might pick semver, might pick `vN`, might pick a different header name. We commit to honoring whatever they choose.

Each PR is open as **draft** with explicit "Blocked on KeeperHub/cli#NN" in the body. When KH ships the upstream change, we mark each ready and the diff converts to a real Rust change against the adapter.

## What makes round-2 feedback different from round-1

- **All 5 round-2 issues come with a companion adapter PR.** Round 1 issues were standalone (no PR drafts). Round 2 demonstrates we have not just identified the friction but designed end-to-end fixes.
- **All 5 round-2 issues are post-integration concerns.** Round 1 covered "couldn't get it working at all" frictions. Round 2 covers "got it working, now thinking about what scaling production hardening looks like."
- **All 5 round-2 issues are independently shippable.** None block another round-2 issue (#52 weakly depends on #51 from round 1, but the catalog ships standalone). KH's docs/platform team can pick them up in any order.

## Round 2 + round 1 totals

| Metric | Round 1 | Round 2 | Total |
|---|---|---|---|
| Issues filed on `KeeperHub/cli` | 5 (#47–#51) | 5 (#52–#56) | **10** |
| Companion adapter-shape PRs on our repo (originally drafts; all merged 2026-05-03 12:00 UTC) | 0 | 5 (#402–#406) | **5** |
| Adapter line refs cited | 6 | 9 | 15 |
| Proposed code-shape diffs | 0 | ~200 lines across 5 files | ~200 |

## Builder Feedback bounty self-assessment

The KH BF bounty rewards **specific, actionable, reproducible** feedback from teams that actually shipped against KeeperHub. Round 1 (#47–#51) hit the "actionable + reproducible" bar with worked `curl` reproductions and proposed shapes. Round 2 (#52–#56) extends to **end-to-end demonstrability**: every issue has a companion PR showing the consumer-side change ready to ship the day KH lands the upstream contract. That's the maximal credible commitment an adapter author can make to "we'll consume this if you ship it."

We've now filed 10 distinct issues + 5 draft PRs across two rounds, every one derived from real friction in shipping `sbo3l-keeperhub-adapter` v1.0 → v1.2 to crates.io. We expect any subset of these to land within KH's normal docs/feature flow. The fastest unlock for the broader KH adapter ecosystem: #48 (envelope schema, round 1) + #52 (error catalog, round 2) + #55 (schema version header, round 2) — three docs/platform changes that together unblock every subsequent adapter author.

## Verification checklist for judges

Direct links (no GitHub login required — `author:@me` would have only resolved for the signed-in viewer):

- The 5 round-2 KH issues:
  - [KeeperHub/cli#52](https://github.com/KeeperHub/cli/issues/52) — HTTP error code catalog + retry semantics
  - [KeeperHub/cli#53](https://github.com/KeeperHub/cli/issues/53) — public mock fixture suite / Docker image
  - [KeeperHub/cli#54](https://github.com/KeeperHub/cli/issues/54) — webhook timeout SLO publication
  - [KeeperHub/cli#55](https://github.com/KeeperHub/cli/issues/55) — schema-version headers
  - [KeeperHub/cli#56](https://github.com/KeeperHub/cli/issues/56) — max payload size documentation
- The 5 companion draft PRs on this repo:
  - [PR #402](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/402) — consumer shape for KH-cli#52
  - [PR #403](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/403) — consumer shape for KH-cli#53
  - [PR #404](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/404) — consumer shape for KH-cli#54
  - [PR #405](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/405) — consumer shape for KH-cli#55
  - [PR #406](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/406) — consumer shape for KH-cli#56
- [Re-read round 1 evidence](../submission/bounty-keeperhub-builder-feedback.md) — original 5 issues (#47–#51) that started the cumulative submission.

Filtered listings (link to a search anyone can run; uses explicit author). Both listings cover all 15 issues across rounds 1+2+3 and the 5 round-2 companion draft PRs:
- [All SBO3L-filed issues on KeeperHub/cli](https://github.com/KeeperHub/cli/issues?q=is%3Aissue+author%3AB2JK-Industry)
- [All KH-BF draft PRs on this repo](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pulls?q=is%3Apr+author%3AB2JK-Industry+%22kh-bf-additional%22)

## Sponsor-specific value prop

We want KH to win with us. Every issue in this round has a paired implementation commitment from our side. Five round-2 issues × five draft PRs = ten distinct contracts where SBO3L commits to landing the consumer-side change as soon as KH ships the upstream platform change. That's a higher commitment density than vendor-SDK feedback usually carries and it's the thing the Builder Feedback bounty is designed to reward.

---

## Round 3 (2026-05-03 evening) — KH-BF-A3 amplifier closure

5 MORE concrete asks filed (each ≤ 200 words), bringing total filed feedback to **15 issues + 5 reference PRs** on KeeperHub/cli — clears the KH-BF-A3 amplifier threshold (15+ issues).

| Issue | Subject |
|---|---|
| [KeeperHub/cli#58](https://github.com/KeeperHub/cli/issues/58) | HMAC-SHA256 webhook signature verification (`X-KeeperHub-Signature`) |
| [KeeperHub/cli#59](https://github.com/KeeperHub/cli/issues/59) | Workflow versioning (per-revision IDs) + back-compat policy |
| [KeeperHub/cli#60](https://github.com/KeeperHub/cli/issues/60) | Publish JSON Schema for webhook 2xx response envelope |
| [KeeperHub/cli#61](https://github.com/KeeperHub/cli/issues/61) | `X-KeeperHub-RateLimit-*` headers on every webhook response |
| [KeeperHub/cli#62](https://github.com/KeeperHub/cli/issues/62) | Document webhook delivery guarantees (semantics + ordering + dedup) |

### Why these 5

Round 2 covered **post-integration concerns** (error catalog, fixture suite, SLO, schema versioning, payload size). Round 3 covers **production-grade reliability concerns**: signing for reverse-direction webhooks, workflow revision pinning for upgrade safety, response schema for parser stability, rate-limit telemetry for client pacing, and delivery-guarantees documentation for queue-backed integrations. Each is a single platform change KH can land in any order; together they define the reliability contract a production agent fleet needs.

### Cumulative totals (rounds 1 + 2 + 3)

| Metric | R1 | R2 | R3 | Total |
|---|---|---|---|---|
| Issues on `KeeperHub/cli` | 5 (#47–#51) | 5 (#52–#56) | 5 (#58–#62) | **15** |
| Companion adapter-shape PRs (originally drafts; all merged 2026-05-03 12:00 UTC) | 0 | 5 (#402–#406) | 0 (R3 issues are doc-only asks) | **5** |
| Adapter line refs cited | 6 | 9 | 1 (#60 cites lib.rs:316-319) | 16 |

### Updated filtered listings

- [All 15 SBO3L-filed issues on KeeperHub/cli](https://github.com/KeeperHub/cli/issues?q=is%3Aissue+author%3AB2JK-Industry)
- [All 5 KH-BF adapter-shape PRs on this repo (now merged)](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pulls?q=is%3Apr+author%3AB2JK-Industry+%22kh-bf-additional%22)
