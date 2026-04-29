# Mandate × ENS — partner one-pager

> ENS is not cosmetic here. It is how a third party finds and verifies an
> agent's mandate.

**Status: target product framing, with the parts that already exist on
`main` clearly separated from the parts that depend on later phases.**

## The pitch in one paragraph

Autonomous agents need a stable public identity that points at *what
authority they hold*, not just *who they are*. Mandate uses ENS text
records as the agent passport registry: the agent's ENS name resolves to
the Mandate endpoint, the active policy hash, the audit root, the proof
URI, and (target) the passport schema and KeeperHub workflow id. This
turns ENS from a name service into a verifiable agent-discovery surface:
a reviewer or sponsor can resolve `research-agent.team.eth`, see the
published policy hash, and compare it to the policy Mandate is actually
running.

## What is implemented today (on `main`, this build)

- ENS adapter `mandate_identity::OfflineEnsResolver` — offline, fixture-
  driven resolver. The 13-gate final demo (gate 7) uses this resolver to
  look up `research-agent.team.eth` and verifies the resolved
  `mandate:policy_hash` matches the receipt's `policy_hash`.
- Multi-agent ENS fixture catalogue:
  [`demo-fixtures/mock-ens-registry.json`](../../demo-fixtures/mock-ens-registry.json)
  with companion guide
  [`demo-fixtures/mock-ens-registry.md`](../../demo-fixtures/mock-ens-registry.md).
- Single-agent runtime input consumed by the demo script:
  [`demo-fixtures/ens-records.json`](../../demo-fixtures/ens-records.json).
- Builder feedback (current): [`FEEDBACK.md` §ENS](../../FEEDBACK.md).

**Resolver source today: `offline-fixture` only.** No live ENS RPC is
called from any code path in this build. The trust badge and operator
console label every ENS reference accordingly.

## What is target (Mandate Passport phase, not on main yet)

These are explicit *targets* documented for the team and for ENS reviewers
— none of them are claimed as shipped:

- **`mandate:*` text-record namespace (target).** A blessed set of agent
  metadata records:
  - `mandate:mcp_endpoint` — target MCP/API surface an agent can ask for
    decisions. The shipped offline fixture currently uses the generic
    `mandate:endpoint` key.
  - `mandate:policy_hash` — canonical hash of the active policy.
  - `mandate:audit_root` — current audit-chain / mock-checkpoint
    commitment (mock today; real onchain later).
  - `mandate:passport_schema` — capsule schema id (`mandate.passport_capsule.v1`,
    target).
  - `mandate:proof_uri` — public capsule/proof URL.
  - `mandate:keeperhub_workflow` — sponsor execution workflow id.
- **Live ENS resolver (`LiveEnsResolver`, future, gated).** Same trait
  surface as the offline resolver, gated behind an explicit
  `MANDATE_ENS_LIVE=1` env-var. Never a silent fallback from offline
  fixture. CI will never require live ENS.
- **Drift detection in proof viewers (target).** When ENS publishes
  policy hash A but Mandate's active policy is hash B, the trust badge
  and operator console must render an explicit failure pill — never a
  fake-OK.

## Why ENS specifically

Text records are a perfect substrate for arbitrary structured agent
metadata — no custom contract needed. The "policy hash matches what is
published" pattern gives reviewers immediate confidence in a single line
of comparison. ENS publishes the commitment; Mandate enforces it.

## What we are asking ENS for (concrete, scoped)

These are the same asks recorded in
[`FEEDBACK.md` §ENS](../../FEEDBACK.md), summarised here:

1. **A blessed text-record namespace for autonomous agents.** Today the
   `mandate:*` prefix is a soft convention; we'd happily move under a
   blessed `agent:*` namespace if the ecosystem standardises one.
   Fragmentation across projects is the bigger risk than naming.
2. **A canonical `policy_commitment` record.** Multiple security tools
   (Mandate plus future analogues) should be able to publish a hash of
   their active policy under one key, instead of each tool inventing its
   own slot.
3. **A canonical `proof_uri` record.** A standardised slot for "where the
   public proof / capsule for this agent lives", so any client can find
   the proof without out-of-band convention.
4. **Guidance for endpoint records on agents.** Where should a policy
   gateway endpoint live — alongside `url` text records, under a
   sub-namespace, etc.? Today the shipped fixture uses
   `mandate:endpoint`; the Passport target would prefer
   `mandate:mcp_endpoint` or a future blessed equivalent.

## What this one-pager will NOT claim

- Mandate **does not** resolve a real ENS testnet/mainnet name in this
  build. Every ENS reference goes through `OfflineEnsResolver` against a
  fixture file, and the source is labelled `offline-fixture`.
- The published `mandate:*` keys are **a soft convention** — ENS does
  not endorse them as a namespace yet.
- ENS **does not** enforce policy. It publishes / discovers a commitment;
  Mandate enforces.
- Mandate Passport capsule + `mandate:passport_schema` are **target
  product framing**, not shipped artefacts in this build. The capsule
  schema (A-side) lands in a later phase; this one-pager will be updated
  to reference the actual schema id once that PR is on `main`.

## Pointers in this repo

- Adapter source: [`crates/mandate-identity/src/`](../../crates/mandate-identity/src/)
- Single-agent runtime input: [`demo-fixtures/ens-records.json`](../../demo-fixtures/ens-records.json)
- Multi-agent catalogue + guide: [`demo-fixtures/mock-ens-registry.json`](../../demo-fixtures/mock-ens-registry.json) / [`demo-fixtures/mock-ens-registry.md`](../../demo-fixtures/mock-ens-registry.md)
- Sponsor demo (offline today): [`demo-scripts/sponsors/ens-agent-identity.sh`](../../demo-scripts/sponsors/ens-agent-identity.sh)
- Production transition checklist (env vars for live resolver): [`docs/production-transition-checklist.md` §ENS](../production-transition-checklist.md#ens-resolver)
- Builder feedback: [`FEEDBACK.md` §ENS](../../FEEDBACK.md)
- Product source of truth: [`docs/product/MANDATE_PASSPORT_SOURCE_OF_TRUTH.md`](../product/MANDATE_PASSPORT_SOURCE_OF_TRUTH.md)
- Reward strategy: [`docs/product/REWARD_STRATEGY.md`](../product/REWARD_STRATEGY.md)
