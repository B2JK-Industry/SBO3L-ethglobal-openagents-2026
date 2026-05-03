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

**Resolver source today: live mainnet ENS + offline fixture (both shipped).** Default path uses `LiveEnsResolver` (real JSON-RPC against PublicNode mainnet/Sepolia); CI uses `OfflineEnsResolver` against the fixture file. Switching is a single trait call, no re-deploy.

### Live mainnet artefacts (verified)

- **`sbo3lagent.eth` mainnet apex** — owner `0xdc7EFA…D231`, resolver `0xF291…AC15` (Public Resolver v3). Five `sbo3l:*` records published: `agent_id`, `endpoint`, `policy_hash` (`e044f13c…`), `audit_root`, `proof_uri`. Verify with: `sbo3l agent verify-ens sbo3lagent.eth --network mainnet --rpc-url https://ethereum-rpc.publicnode.com` → verdict PASS.
- **Sepolia OffchainResolver** at `0x87e99508C222c6E419734CACbb6781b8d282b1F6` (PRs #383, #386, #396, #411). Implements ENSIP-25 / EIP-3668 CCIP-Read with a Vercel-hosted gateway at `https://sbo3l-ccip.vercel.app/api/{sender}/{data}.json` returning EIP-712-signed text records. End-to-end CLI follower: `sbo3l agent verify-ens research-agent.sbo3lagent.eth --network sepolia` returns the actual records via OffchainLookup loop (R20 PR #446).
- **Sepolia subnames live**: `research-agent.sbo3lagent.eth`, registered via direct ENS Registry `setSubnodeRecord` (Daniel owns the apex, no third-party registrar; we evaluated Durin and dropped it).
- **ERC-8004 IdentityRegistry** deployed on Sepolia at `0x600c10dE2fd5BB8f3F47cd356Bcb80289845Db37` (PR #358). `sbo3l agent register` writes via the registry.
- **Sepolia AnchorRegistry / SubnameAuction / ReputationBond / ReputationRegistry** all deployed + ABI-callable (verified via `sbo3l doctor --extended`).

### ENS-MC narrative artefacts shipped

- **Trust DNS Manifesto** (5,000 words, RFC-style normative MUST/SHOULD/MAY): [`docs/ens/trust-dns-manifesto.md`](../ens/trust-dns-manifesto.md) (PR #388).
- **ENSIP-N draft** (366 lines) proposing the `sbo3l:*` namespace formally: [`docs/ENSIP-N-DRAFT.md`](../ENSIP-N-DRAFT.md).
- **Kevin's caveat preempted** (R19 Task A.5 URGENT, PR #421) — locks Resolver address as part of policy commitment hash so any switch to a different Resolver fails capsule verification.

### Drift detection — shipped, not target

When ENS publishes `policy_hash` A but SBO3L's active policy is hash B, **`sbo3l agent verify-ens` fails closed** (verdict ≠ PASS). The trust-badge proof viewer and operator console render an explicit `policy_hash_drift` failure pill — never a fake-OK. The capsule verifier additionally rejects any capsule whose embedded `policy_snapshot.hash` doesn't byte-match the on-chain record.

## What is target (post-submission roadmap)

These are explicit *targets* — none claimed as shipped:

- **Mainnet `sbo3l.eth` apex + 60 subnames** (ENS-AGENT-A1 amplifier, ~$200 mainnet ETH) — would lift ENS Best Integration AI Agents track from 3rd → 1st.
- **Mainnet OffchainResolver deploy** at `ccip.sbo3l.dev` — currently mainnet apex points at Public Resolver with regular text records; mainnet OR deploy ~$10 ETH. Operator-gated (record-migration risk on the existing live records).
- **ERC-8004 mainnet registration** — Sepolia deploy is shipped; mainnet path documented in [`docs/erc8004-integration.md`](../erc8004-integration.md).
- **Formal ENSIP submission to `ensdomains/docs`** — draft is ready; awaiting one more pass on the sbo3l:* schema after community feedback.

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

- The default CI path uses `OfflineEnsResolver` against a fixture file (deterministic, network-free); the production path uses `LiveEnsResolver` and is verified end-to-end against `sbo3lagent.eth` mainnet during the submission window. Both paths share the same trait surface.
- The published `sbo3l:*` keys are **a soft convention today** — the ENSIP-N draft proposes formalising them; ENS does not yet endorse them as a blessed namespace.
- ENS **does not** enforce policy. It publishes / discovers a commitment; SBO3L enforces.
- The mainnet apex `sbo3lagent.eth` currently points at PublicResolver with regular text records. A mainnet OffchainResolver deploy is a target (post-submission), not shipped.

## Pointers in this repo

- Adapter source: [`crates/sbo3l-identity/src/`](../../crates/sbo3l-identity/src/)
- Single-agent runtime input: [`demo-fixtures/ens-records.json`](../../demo-fixtures/ens-records.json)
- Multi-agent catalogue + guide: [`demo-fixtures/mock-ens-registry.json`](../../demo-fixtures/mock-ens-registry.json) / [`demo-fixtures/mock-ens-registry.md`](../../demo-fixtures/mock-ens-registry.md)
- Sponsor demo (offline today): [`demo-scripts/sponsors/ens-agent-identity.sh`](../../demo-scripts/sponsors/ens-agent-identity.sh)
- Production transition checklist (env vars for live resolver): [`docs/production-transition-checklist.md` §ENS](../production-transition-checklist.md#ens-resolver)
- Builder feedback: [`FEEDBACK.md` §ENS](../../FEEDBACK.md)
- Product source of truth: [`docs/product/SBO3L_PASSPORT_SOURCE_OF_TRUTH.md`](../product/SBO3L_PASSPORT_SOURCE_OF_TRUTH.md)
- Reward strategy: [`docs/product/REWARD_STRATEGY.md`](../product/REWARD_STRATEGY.md)
