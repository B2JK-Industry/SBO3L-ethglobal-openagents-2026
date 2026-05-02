# SBO3L × ENS — partner one-pager

> ENS is not cosmetic here. It is how a third party finds and verifies an
> agent's mandate.

**Status: target product framing, with the parts that already exist on
`main` clearly separated from the parts that depend on later phases.**

## The pitch in one paragraph

Autonomous agents need a stable public identity that points at *what
authority they hold*, not just *who they are*. SBO3L uses ENS text
records as the agent passport registry: the agent's ENS name resolves to
the SBO3L endpoint, the active policy hash, the audit root, the proof
URI, and (target) the passport schema and KeeperHub workflow id. This
turns ENS from a name service into a verifiable agent-discovery surface:
a reviewer or sponsor can resolve `research-agent.team.eth`, see the
published policy hash, and compare it to the policy SBO3L is actually
running.

## Why this isn't another self-claimed text record

Kevin (ENS team) recently flagged in #ens the obvious risk for any
agent-identity-via-ENS scheme: *a user can set whatever github/X txt
record they want — it's playing the reputation system.* That argument
disqualifies most naive ENS-as-identity pitches.

SBO3L's `sbo3l:policy_hash` is the rebuttal — it's not a user claim,
it's a JCS+SHA-256 commitment to the canonical policy snapshot the live
engine actually enforces. The CLI command `sbo3l agent verify-ens
<name>` performs a drift check on every call: if the published hash
doesn't match the engine's runtime policy, the call fails closed with
`policy_hash.drift_detected`. **The text record is verifiable, not
claimed.** Source-of-truth: `crates/sbo3l-identity/src/verify_ens.rs`.

## What is implemented today (on `main`, this build)

- ENS adapter `sbo3l_identity::OfflineEnsResolver` — offline, fixture-
  driven resolver. The 13-gate final demo (gate 7) uses this resolver to
  look up `research-agent.team.eth` and verifies the resolved
  `sbo3l:policy_hash` matches the receipt's `policy_hash`.
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

## What is target (SBO3L Passport phase, not on main yet)

These are explicit *targets* documented for the team and for ENS reviewers
— none of them are claimed as shipped:

- **`sbo3l:*` text-record namespace (target).** A blessed set of agent
  metadata records:
  - `sbo3l:mcp_endpoint` — target MCP/API surface an agent can ask for
    decisions. The shipped offline fixture currently uses the generic
    `sbo3l:endpoint` key.
  - `sbo3l:policy_hash` — canonical hash of the active policy.
  - `sbo3l:audit_root` — current audit-chain / mock-checkpoint
    commitment (mock today; real onchain later).
  - `sbo3l:passport_schema` — capsule schema id (`sbo3l.passport_capsule.v1`,
    target).
  - `sbo3l:proof_uri` — public capsule/proof URL.
  - `sbo3l:keeperhub_workflow` — sponsor execution workflow id.
- **Live ENS resolver (`LiveEnsResolver`, future, gated).** Same trait
  surface as the offline resolver, gated behind an explicit
  `SBO3L_ENS_LIVE=1` env-var. Never a silent fallback from offline
  fixture. CI will never require live ENS.
- **Drift detection in proof viewers (target).** When ENS publishes
  policy hash A but SBO3L's active policy is hash B, the trust badge
  and operator console must render an explicit failure pill — never a
  fake-OK.

## Why ENS specifically

Text records are a perfect substrate for arbitrary structured agent
metadata — no custom contract needed. The "policy hash matches what is
published" pattern gives reviewers immediate confidence in a single line
of comparison. ENS publishes the commitment; SBO3L enforces it.

## What we are asking ENS for (concrete, scoped)

These are the same asks recorded in
[`FEEDBACK.md` §ENS](../../FEEDBACK.md), summarised here:

1. **A blessed text-record namespace for autonomous agents.** Today the
   `sbo3l:*` prefix is a soft convention; we'd happily move under a
   blessed `agent:*` namespace if the ecosystem standardises one.
   Fragmentation across projects is the bigger risk than naming.
2. **A canonical `policy_commitment` record.** Multiple security tools
   (SBO3L plus future analogues) should be able to publish a hash of
   their active policy under one key, instead of each tool inventing its
   own slot.
3. **A canonical `proof_uri` record.** A standardised slot for "where the
   public proof / capsule for this agent lives", so any client can find
   the proof without out-of-band convention.
4. **Guidance for endpoint records on agents.** Where should a policy
   gateway endpoint live — alongside `url` text records, under a
   sub-namespace, etc.? Today the shipped fixture uses
   `sbo3l:endpoint`; the Passport target would prefer
   `sbo3l:mcp_endpoint` or a future blessed equivalent.

## What this one-pager will NOT claim

- SBO3L **does not** resolve a real ENS testnet/mainnet name in this
  build. Every ENS reference goes through `OfflineEnsResolver` against a
  fixture file, and the source is labelled `offline-fixture`.
- The published `sbo3l:*` keys are **a soft convention** — ENS does
  not endorse them as a namespace yet.
- ENS **does not** enforce policy. It publishes / discovers a commitment;
  SBO3L enforces.
- SBO3L Passport capsule + `sbo3l:passport_schema` are **target
  product framing**, not shipped artefacts in this build. The capsule
  schema (A-side) lands in a later phase; this one-pager will be updated
  to reference the actual schema id once that PR is on `main`.

## Pointers in this repo

- Adapter source: [`crates/sbo3l-identity/src/`](../../crates/sbo3l-identity/src/)
- Single-agent runtime input: [`demo-fixtures/ens-records.json`](../../demo-fixtures/ens-records.json)
- Multi-agent catalogue + guide: [`demo-fixtures/mock-ens-registry.json`](../../demo-fixtures/mock-ens-registry.json) / [`demo-fixtures/mock-ens-registry.md`](../../demo-fixtures/mock-ens-registry.md)
- Sponsor demo (offline today): [`demo-scripts/sponsors/ens-agent-identity.sh`](../../demo-scripts/sponsors/ens-agent-identity.sh)
- Production transition checklist (env vars for live resolver): [`docs/production-transition-checklist.md` §ENS](../production-transition-checklist.md#ens-resolver)
- Builder feedback: [`FEEDBACK.md` §ENS](../../FEEDBACK.md)
- Product source of truth: [`docs/product/SBO3L_PASSPORT_SOURCE_OF_TRUTH.md`](../product/SBO3L_PASSPORT_SOURCE_OF_TRUTH.md)
- Reward strategy: [`docs/product/REWARD_STRATEGY.md`](../product/REWARD_STRATEGY.md)
