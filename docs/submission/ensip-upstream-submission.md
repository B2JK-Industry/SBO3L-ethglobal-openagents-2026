# ENSIP-N upstream submission packet

> **Recipient:** Dhaiwat Pandya (ENS DAO contributor; SBO3L ENS
> bounty contact). CC: Simon Brown (`ses.eth`, technical reviewer).
> **Submission target:** [`ensdomains/ensips`](https://github.com/ensdomains/ensips)
> upstream PR, post-ETHGlobal Open Agents 2026.
> **Status:** ready-for-feedback draft. The full ENSIP body lives at
> [`docs/proof/ensip-draft-reputation.md`](../proof/ensip-draft-reputation.md).
> This document is the *cover letter* + iteration plan.

## Summary for Dhaiwat

We'd like to propose a small, focused ENSIP standardising
**`reputation_score`** as a portable text-record key — decimal
integer 0..=100 ASCII, with optional sibling records for
methodology, proof, and freshness pointers. The full draft is in
the repo
([`docs/proof/ensip-draft-reputation.md`](../proof/ensip-draft-reputation.md))
and we have a working reference implementation
([`crates/sbo3l-identity/src/reputation_publisher.rs`](../../crates/sbo3l-identity/src/reputation_publisher.rs))
plus a cross-publisher aggregation primitive
([`crates/sbo3l-policy/src/cross_chain_reputation.rs`](../../crates/sbo3l-policy/src/cross_chain_reputation.rs))
shipped during the hackathon.

The five-paragraph version is below; the full draft ahead of an
upstream PR is what we'd send to the ENSIP repo.

## Why a text-record convention is the right shape

The autonomous-agent ecosystem will produce reputation signals
from many sources (audit-chain operators, ERC-8004 registries,
attestation services, third-party reputation oracles). Without a
shared key, every consumer is forced to know each publisher's
bespoke ABI to read a signal that is *fundamentally one number*.
ENS already gives us the namespace and the read primitive
(`text(node, key)`); the missing piece is the convention.

`reputation_score` is intentionally minimal. Decimal integer
0..=100, ASCII. Reads cleanly in `viem.getEnsText`, ENS App, raw
`cast text` — zero new decoders. A consumer reading this record
across publishers using different methodologies can disambiguate
via the sibling `reputation_score_method` URI pointer; that
preserves the "multiple opinions per name" pattern DNS uses for
`TXT` records (`v=DKIM1`, `v=SPF1`, …).

## What we're explicitly NOT proposing

- **Not standardising the scoring method.** Two publishers with
  competing methodologies coexist via the sibling pointer; the
  ENSIP refuses to bake any single methodology in.
- **Not competing with ERC-8004.** ERC-8004 is the right substrate
  when consumers want gas-cheap on-chain *checks*. This ENSIP is
  the right substrate when consumers want human-readable, off-chain
  *checks* without committing to a specific registry contract per
  chain. The two compose: an ERC-8004 registry MAY mirror its
  score into an agent's ENS `reputation_score` for off-chain
  readers; conversely, an ENSIP-conformant publisher MAY anchor
  into an ERC-8004 registry for on-chain consumers.
- **Not requiring CCIP-Read.** The convention works equally well
  for static on-chain records and dynamic CCIP-Read-served
  records. Publishers choose based on update frequency.

## What this unblocks for SBO3L specifically

We're publishing reputation today as `sbo3l:reputation_score`
(SBO3L-namespaced). Once the ENSIP lands we'd mirror to both
`sbo3l:reputation_score` (private namespace, semver-stable for
existing consumers) and `reputation_score` (public namespace, the
ENSIP-conformant key). Consumers who want a portable signal read
the public key; SBO3L-specific consumers continue reading the
private one. Zero migration cost on either side.

The cross-chain reputation aggregator
([`crates/sbo3l-policy/src/cross_chain_reputation.rs`](../../crates/sbo3l-policy/src/cross_chain_reputation.rs))
already operates over the *generic* shape — it doesn't care which
namespace the score lives in. The hard part of the upstream PR is
the wire-format agreement, not the consumer side.

## Suggested upstream-PR shape

We'd open one ENSIP PR with three logical commits:

1. **Spec body** — the `docs/proof/ensip-draft-reputation.md`
   content adapted to the `ensdomains/ensips` template (front
   matter: status `Draft`, type `ENSIP`, category `Application`).
2. **Reference-impl pointer** — link to SBO3L's
   `reputation_publisher.rs` + the cross-chain aggregator. We're
   committing to maintain these as the canonical implementation
   reference until either (a) the ENSIP graduates to Final and
   consumers point at multiple impls, or (b) we abandon the
   project in writing.
3. **Test-vector file** — at least 8 reproducible JSON test vectors
   covering the validation rules (4 accepted + 12 rejected forms
   from the draft's "Test cases" section). Living in the ENSIP
   repo so future implementers can validate their decoder.

## Iteration plan with Dhaiwat / Simon

We'd appreciate one round of feedback on:

1. **Naming** — `reputation_score` vs a longer namespaced form
   like `agent.reputation` vs `eip-N.reputation_score`. The ENS
   community's namespacing convention here is what we'd defer to.
2. **Sibling records** — are
   `reputation_score_method` / `_proof` / `_updated` the right
   set, or should we collapse into a single envelope-pointer
   record?
3. **Range / encoding** — 0..=100 integer vs 0..=10000 (basis
   points) vs floating-point string. We're committing to integer
   0..=100 in the draft; happy to revisit if the ENS community
   prefers basis points for downstream comparison fidelity.
4. **Composition with ERC-8004** — we've drafted the composition
   story but haven't run it past the ERC-8004 authors. A
   coordinated ENSIP + ERC-8004-side note would land cleaner than
   either alone.

After feedback merges, we'd open the upstream PR. We expect ~1
review cycle before requesting `Last Call`; the spec is small
enough that the iteration window should be measured in weeks not
months.

## Timeline

- **Hackathon close** (2026-05-02) — this packet ready for handoff
  to Dhaiwat.
- **+1 week** — feedback received, in-repo draft updated.
- **+2 weeks** — upstream PR opened against `ensdomains/ensips`.
- **+4 weeks** — `Last Call` requested if no major spec changes.

## Contact

- Daniel Babjak — `babjak_daniel@hotmail.com` — ENSIP author.
- Repository — `https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026`
- ENS bounty narrative — [`docs/submission/bounty-ens-most-creative-final.md`](bounty-ens-most-creative-final.md)
- Reference impl — [`crates/sbo3l-identity/src/reputation_publisher.rs`](../../crates/sbo3l-identity/src/reputation_publisher.rs)
- Cross-chain aggregator — [`crates/sbo3l-policy/src/cross_chain_reputation.rs`](../../crates/sbo3l-policy/src/cross_chain_reputation.rs)

## Files attached

- [`docs/proof/ensip-draft-reputation.md`](../proof/ensip-draft-reputation.md)
  — the formal ENSIP body in upstream-PR shape.
- [`docs/proof/ens-technical-paper.md`](../proof/ens-technical-paper.md)
  — context for the broader SBO3L ENS architecture (3000-word, ENS
  Labs audience).

## Anti-goal: what NOT to discuss in the first iteration

To keep the upstream review scoped, we'd defer these to follow-up
ENSIPs:

- A reputation-aggregation algorithm standard. (Out of scope; each
  consumer composes their own.)
- A cross-chain attestation schema. (Tracked separately as
  T-3-8 / `cross_chain.rs`; would land as its own ENSIP if
  community appetite exists.)
- Token-gated agent identity. (Tracked separately as P5 / round 9
  `token_gate.rs`; orthogonal to the reputation convention.)

Each of these is a separate proposal with its own merits; bundling
them with the reputation key would slow the smaller, easier
proposal down.

---

*This packet is markdown-only for now — submission upstream is a
post-hackathon action item. The repo is the canonical artefact
trail; everything else is a pointer.*
