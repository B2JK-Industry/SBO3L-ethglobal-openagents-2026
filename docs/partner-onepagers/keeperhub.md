# SBO3L × KeeperHub — partner one-pager

> **KeeperHub executes. SBO3L proves the execution was authorised.** Two complementary layers in the same agent stack — the policy boundary and the execution substrate — designed from the start to compose without rewriting either side.

## How SBO3L plugs into KeeperHub

```
agent  ──APRP──▶  [SBO3L boundary]  ──signed PolicyReceipt + sbo3l_* envelope──▶  [KeeperHub workflow webhook]  ──executionId──▶  [KeeperHub execution row]
                       │                                                                                                                       │
                       └─ hash-chained audit ─ sbo3l.audit_bundle.v1 ─ sbo3l.passport_capsule.v1 (target) ─ optional sbo3l_passport_uri ──┘
```

Five **specific shapes** the KeeperHub team could merge or build on are catalogued in [`docs/keeperhub-integration-paths.md`](../keeperhub-integration-paths.md), each independently small and independently reviewable:

| # | Shape | Adoption cost on KeeperHub side |
|---|---|---|
| **IP-1** | `sbo3l_*` upstream-proof envelope fields on the workflow webhook | 4–5 optional string fields, echo on lookup |
| **IP-2** | Public submission/result envelope JSON Schema | One JSON Schema file under your docs |
| **IP-3** | `keeperhub.lookup_execution(execution_id)` MCP tool | One MCP tool definition + thin handler |
| **IP-4** | Standalone `sbo3l-keeperhub-adapter` Rust crate | Listing on your "integrations" page; crates.io publication target |
| **IP-5** | SBO3L Passport capsule URI on the execution row | One optional string column |

Stacking the shapes gives **end-to-end offline auditability** of every KeeperHub execution that flowed through SBO3L — anywhere in the chain, an auditor with the right keys can reconstruct what was authorised, who authorised it, which policy applied, and where the audit chain says it sits, without trusting any single party.

## Why this pairing specifically

KeeperHub's framing — *the execution layer for AI agents operating onchain* — maps cleanly onto SBO3L's `GuardedExecutor` trait. The integration is a thin adapter, not a rewrite. KeeperHub records *what was executed*; SBO3L records *why it was authorised*. The IP-1 envelope is the bridge between those two records, and SBO3L produces every byte of it for free as part of its existing pipeline (canonical request hash, policy hash, Ed25519 receipt signature, ULID audit event id).

A KeeperHub auditor today reading an execution row has no cryptographic link back to whoever authorised the action. With IP-1 alone, that link becomes one offline verification. With IP-1 + IP-5, that link becomes one HTTP fetch.

## What is implemented today (on `main`, this build)

- **Adapter trait `GuardedExecutor` and concrete `KeeperHubExecutor`** ([`crates/sbo3l-keeperhub-adapter/`](../../crates/sbo3l-keeperhub-adapter/), re-exported by `sbo3l-execution`) with two constructors:
  - `KeeperHubExecutor::local_mock()` — used in every demo path today. Returns a deterministic `kh-<ULID>` execution_ref and prints `mock: true` in demo output.
  - `KeeperHubExecutor::live()` — present as a constructor; intentionally `BackendOffline` until a stable submission/result schema and credentials are available. **No live network call is made in this build.**
- **Production-shaped runner step 6** walks the allow → KeeperHub mock path end-to-end. Step 6 also walks the prompt-injection deny path and proves the denied receipt never reaches the sponsor (`keeperhub_refused: true`, visible in transcript and operator console).
- **Hash-chained audit log + offline-verifiable bundle** — every Allow decision produces a signed `PolicyReceipt` linked by ULID audit-event-id into a hash-chained audit log. `sbo3l audit export` packages the receipt + audit-chain prefix into a single `sbo3l.audit_bundle.v1` JSON file; `sbo3l audit verify-bundle` re-derives every claim from that file alone, no daemon required.
- **SBO3L Passport capsule schema + verifier** (PR [#42](https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026/pull/42)) — `sbo3l.passport_capsule.v1` is the IP-5 artefact; the schema and verifier are on `main`, productisation tracked in [`docs/product/SBO3L_PASSPORT_BACKLOG.md`](../product/SBO3L_PASSPORT_BACKLOG.md).
- **Live-integration spike** ([`docs/keeperhub-live-spike.md`](../keeperhub-live-spike.md)) — read-only design for the live PR, including the wire format SBO3L would post, the eight open questions for the KeeperHub team, the test strategy that keeps CI offline, and the file-by-file shopping list (~250 lines of Rust).
- **Builder feedback (concrete asks, not abstract complaints)** — [`FEEDBACK.md` §KeeperHub](../../FEEDBACK.md) lists the JSON Schema, the four `sbo3l_*` fields, the lookup endpoint / MCP tool, the optional response headers, and the token-prefix / webhook-signing clarifications, each with rationale and impact.

## What is target (SBO3L Passport phase + live KeeperHub)

These are explicit *targets* — none claimed as shipped:

- **SBO3L Passport capsule end-to-end** — schema + verifier exist; producing the capsule from a real KeeperHub execution depends on the live wiring below.
- **`KeeperHubExecutor::live()` actually calling KeeperHub** — wired through [`docs/keeperhub-live-spike.md` §Target shape](../keeperhub-live-spike.md). Gated behind `SBO3L_KEEPERHUB_LIVE=1`, never a silent fallback from mock. CI never sets the flag.
- **`sbo3l-keeperhub-adapter` extracted as standalone workspace crate** — IP-4 above; the adapter is structurally independent of the rest of the workspace today. Crates.io publication remains target.

## What we are asking for (concrete, scoped)

1. **Public submission/result envelope schema** — one JSON Schema 2020-12 file under your docs.
2. **First-class upstream proof fields on submission** — the four (target: five) `sbo3l_*` envelope fields detailed in IP-1.
3. **Documented `executionId → status / run-log lookup`** — GET path or MCP tool.
4. **Optional webhook headers from KeeperHub → caller** — `X-SBO3L-Receipt-Signature` / `X-SBO3L-Policy-Hash` for signed callbacks.
5. **Token-prefix naming clarity** — short "which token does which thing" page covering `kh_*` vs `wfb_*`.
6. **Webhook signing / callback authenticity** semantics so an inbound execution result can be trusted without a side-channel.

The same six asks live in [`FEEDBACK.md` §KeeperHub](../../FEEDBACK.md) with rationale and worked examples. The eight open implementation questions (schema, token model, lookup, rate limit, idempotency, sync vs async, response headers, callback signing) live in [`docs/keeperhub-live-spike.md` §Open questions](../keeperhub-live-spike.md).

## What this one-pager will NOT claim

- SBO3L **does not** call a real KeeperHub endpoint in this build.
- The mock `kh-<ULID>` execution_ref **is not** a real KeeperHub `executionId`.
- KeeperHub **does not** verify SBO3L receipts today; the IP-1 envelope is a target for live integration, not a current KeeperHub-side feature.
- SBO3L Passport capsule production **is** schema-defined and verifier-tested on `main`; producing capsules from live KeeperHub executions is gated on the live wiring landing.

Honest disclosure stays in every demo output (`mock: true` lines, `keeperhub_refused: true` on deny path) and in every doc that references the integration.

## Pointers in this repo

- **Concrete integration paths (IP-1 … IP-5):** [`docs/keeperhub-integration-paths.md`](../keeperhub-integration-paths.md) ← **start here for the "merge or build on" answer**
- Adapter source: [`crates/sbo3l-keeperhub-adapter/`](../../crates/sbo3l-keeperhub-adapter/)
- Sponsor demo (mock today): [`demo-scripts/sponsors/keeperhub-guarded-execution.sh`](../../demo-scripts/sponsors/keeperhub-guarded-execution.sh)
- Production-shaped runner step 6 walks both allow and deny paths: [`demo-scripts/run-production-shaped-mock.sh`](../../demo-scripts/run-production-shaped-mock.sh)
- Live-spike design notes: [`docs/keeperhub-live-spike.md`](../keeperhub-live-spike.md)
- Production transition checklist (env vars / endpoints / credentials): [`docs/production-transition-checklist.md` §KeeperHub](../production-transition-checklist.md#keeperhub-guarded-execution)
- Builder feedback: [`FEEDBACK.md` §KeeperHub](../../FEEDBACK.md)
- Product source of truth: [`docs/product/SBO3L_PASSPORT_SOURCE_OF_TRUTH.md`](../product/SBO3L_PASSPORT_SOURCE_OF_TRUTH.md)
- Reward strategy: [`docs/product/REWARD_STRATEGY.md`](../product/REWARD_STRATEGY.md)
