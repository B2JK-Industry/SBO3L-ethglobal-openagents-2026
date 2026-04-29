# Mandate × KeeperHub — partner one-pager

> KeeperHub executes. Mandate proves the execution was authorized.

**Status: target product framing, with the parts that already exist on
`main` clearly separated from the parts that depend on later phases.**

## The pitch in one paragraph

Autonomous agents can be wrong. KeeperHub gives them a reliable execution
substrate; Mandate is the upstream policy boundary that decides whether the
execution should happen and signs a receipt that ties the KeeperHub run
back to a specific request, policy, budget, and audit-chain position.
Together they make agent execution accountable: every KeeperHub
`executionId` can carry a cryptographic explanation of *why* it was
allowed.

## What is implemented today (on `main`, this build)

- Adapter trait `GuardedExecutor` and concrete `KeeperHubExecutor`
  (`crates/mandate-execution/src/keeperhub.rs`) with two constructors:
  - `KeeperHubExecutor::local_mock()` — used in every demo path today.
    Returns a deterministic `kh-<ULID>` execution_ref and prints
    `mock: true` in demo output.
  - `KeeperHubExecutor::live()` — present as a constructor; intentionally
    `BackendOffline` until a stable submission/result schema and
    credentials are available. **No live network call is made in this
    build.**
- Production-shaped runner step 6 walks the allow → KeeperHub mock path
  end-to-end. Step 6 also walks the prompt-injection deny path and proves
  the denied receipt never reaches the sponsor (`keeperhub_refused: true`,
  visible in transcript and operator console).
- KeeperHub live-integration design notes: [`docs/keeperhub-live-spike.md`](../keeperhub-live-spike.md).
- Builder feedback (current): [`FEEDBACK.md` §KeeperHub](../../FEEDBACK.md).

## What is target (Mandate Passport phase, not on main yet)

These are explicit *targets* documented for the team and for KeeperHub's
review — none of them are claimed as shipped:

- **Mandate Passport capsule (target)** — a single JSON artefact
  (`mandate.passport_capsule.v1`, schema/verifier owned by the A-side
  Passport CLI work) that records, in one file, the request, decision,
  KeeperHub `execution_ref`, audit event, and checkpoint. Until the
  capsule schema lands on `main`, no UI or doc claims a capsule was
  produced.
- **Proof handoff envelope (target)** — a documented set of fields
  Mandate would attach when calling a real KeeperHub workflow webhook:
  - `mandate_request_hash` — JCS-canonical SHA-256 of the APRP.
  - `mandate_policy_hash` — canonical hash of the active policy.
  - `mandate_receipt_signature` — Ed25519 signature of the policy receipt.
  - `mandate_audit_event_id` — ULID of the audit-chain event.
  - `mandate_passport_capsule_hash` — content hash of the capsule, once
    capsule schema lands.

  This is the smallest set we believe lets a KeeperHub auditor reading an
  execution row reconnect to the upstream Mandate decision without
  out-of-band correlation. It is not implemented in this build because
  the live KeeperHub path is intentionally stubbed.
- **Live KeeperHub call (future, gated)** — would be wired through
  `KeeperHubExecutor::live()` and exposed via an explicit
  `MANDATE_KEEPERHUB_LIVE=1` env-var gate, never as a silent fallback
  from mock. CI will never require it. **No live KeeperHub call is made
  in this build.**

## Why KeeperHub specifically

KeeperHub's framing — execution layer for AI agents — maps cleanly onto
Mandate's `GuardedExecutor` trait. The integration is a thin adapter, not
a rewrite. KeeperHub's audit trail and Mandate's hash-chained audit log
are complementary: KeeperHub records *what was executed*, Mandate records
*why it was authorised*. The proof handoff envelope above is the bridge.

## What we are asking KeeperHub for (concrete, scoped)

These are the same asks recorded in
[`FEEDBACK.md` §KeeperHub](../../FEEDBACK.md), summarised here for review
context:

1. **Public submission/result envelope schema.** Third-party policy
   engines could validate locally before submission; mismatches surface
   at the policy boundary, not over the wire.
2. **Documented `executionId` → status / run-log lookup.** Either a GET
   path or an MCP tool. Mandate would call this from the operator console
   to render execution status next to the audit-bundle verification panel.
3. **First-class upstream proof fields on submission.** Native,
   schema-blessed slots for the five `mandate_*` fields above so
   KeeperHub's audit trail can re-emit them on the result side and in
   workflow logs.
4. **Optional webhook headers from KeeperHub → caller.** When a workflow
   webhook fires back to a Mandate-style consumer:
   - `X-Mandate-Receipt-Signature: <hex>`
   - `X-Mandate-Policy-Hash: <hex>`

   so the consumer can verify the upstream proof in one network round trip.
5. **Token-prefix naming clarity.** The `kh_*` vs `wfb_*` prefix split
   (KeeperHub-native API tokens vs workflow-webhook tokens) is not obvious
   from outside the docs. A short "which token does which thing" page
   with worked examples would shave significant integration time.
6. **Webhook signing / callback authenticity** semantics so a Mandate
   operator can trust an inbound execution result without a side-channel.

## What this one-pager will NOT claim

- Mandate **does not** call a real KeeperHub endpoint in this build.
- The mock `kh-<ULID>` execution_ref **is not** a real KeeperHub
  `executionId`.
- KeeperHub **does not** verify Mandate receipts today; the proof
  envelope above is a target for live integration, not a current
  KeeperHub-side feature.
- Mandate Passport capsule is **target product framing**, not a shipped
  artefact in this build. The Passport CLI + verifier (A-side) lands in a
  later phase; this one-pager will be updated to reference the actual
  schema/verifier once that PR is on `main`.

## Pointers in this repo

- Adapter source: [`crates/mandate-execution/src/keeperhub.rs`](../../crates/mandate-execution/src/keeperhub.rs)
- Sponsor demo (mock today): [`demo-scripts/sponsors/keeperhub-guarded-execution.sh`](../../demo-scripts/sponsors/keeperhub-guarded-execution.sh)
- Production-shaped runner step 6 walks both allow and deny paths: [`demo-scripts/run-production-shaped-mock.sh`](../../demo-scripts/run-production-shaped-mock.sh)
- Live-spike design notes: [`docs/keeperhub-live-spike.md`](../keeperhub-live-spike.md)
- Production transition checklist (env vars / endpoints / credentials): [`docs/production-transition-checklist.md` §KeeperHub](../production-transition-checklist.md#keeperhub-guarded-execution)
- Builder feedback: [`FEEDBACK.md` §KeeperHub](../../FEEDBACK.md)
- Product source of truth: [`docs/product/MANDATE_PASSPORT_SOURCE_OF_TRUTH.md`](../product/MANDATE_PASSPORT_SOURCE_OF_TRUTH.md)
- Reward strategy: [`docs/product/REWARD_STRATEGY.md`](../product/REWARD_STRATEGY.md)
