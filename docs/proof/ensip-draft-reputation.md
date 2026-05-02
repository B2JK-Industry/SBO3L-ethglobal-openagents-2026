# ENSIP Draft: `reputation_score` text-record convention

| Field         | Value                                                                       |
|---------------|-----------------------------------------------------------------------------|
| **Title**     | A standard text-record key for portable agent reputation scores             |
| **Status**    | Draft (pre-submission) — to be opened upstream after ETHGlobal Open Agents 2026 |
| **Author**    | Daniel Babjak (`babjak_daniel@hotmail.com`); SBO3L team, ETHGlobal Open Agents 2026 |
| **Reference impl** | [`crates/sbo3l-identity/src/reputation_publisher.rs`](../../crates/sbo3l-identity/src/reputation_publisher.rs) (T-4-6, PR #201, merged) + [`crates/sbo3l-policy/src/cross_chain_reputation.rs`](../../crates/sbo3l-policy/src/cross_chain_reputation.rs) (T-3-9, this PR) |
| **Created**   | 2026-05-02                                                                  |

## Abstract

This ENSIP defines a single, portable text-record key —
`reputation_score` — whose value is a decimal integer in the
inclusive range `0..=100`, encoding a verifiable reputation signal
for the ENS name's owning entity. It is intended to be set on any
ENS name representing an actor whose past behaviour can be summarised
as a one-dimensional signal: an autonomous agent, a service operator,
a publisher, or a wallet.

The ENSIP does not standardise *how* the score is computed. It
standardises only the **wire format** so consumers can read scores
across implementations without bespoke decoders. A companion
*interpretation contract* — pinned via a sibling text record
`reputation_score_method` — names the algorithm whose output is
written to `reputation_score`, allowing competing methodologies to
coexist without corrupting the read path.

## Motivation

### Reputation as a public commitment

Existing reputation systems anchor signal off-chain (GitHub stars,
npm downloads, marketplace ratings) or in bespoke on-chain
contracts (Karma, Optimism Attestations, ERC-8004 reputation
registries). Each silos the reputation behind a custom read API.
For the autonomous-agent ecosystem specifically, this means:

- A delegating agent reading "should I trust this peer?" must
  query N different reputation surfaces.
- A reputation publisher targeting M consumers must push to M
  bespoke registries.
- An auditor building a reputation aggregator (e.g. across chains
  per [T-3-9](../../crates/sbo3l-policy/src/cross_chain_reputation.rs))
  must encode every source's bespoke ABI.

ENS solves all three by making the reputation a *text record on the
named entity itself*. The read path is `text(node, "reputation_score")` —
the same path every ENS-aware client already implements.

### Why text-record-as-convention

Compared with the alternatives:

- **A new resolver method (`reputationOf(node)`)**: faster
  single-call reads, but every consumer needs an ABI extension
  and ENSIP-10 wildcard semantics already give us cheap reads via
  Universal Resolver (T-4-5).
- **An off-chain registry**: requires a trusted service, doesn't
  compose with CCIP-Read.
- **An on-chain reputation contract**: requires a custom address
  per network, doesn't reuse existing infrastructure.

The text-record path costs zero new infrastructure. `viem.getEnsText`
already reads the value as a plain string; ENS App already displays
text records; any ENSIP-10-aware library handles the resolution.

## Specification

### Required text record

| Key                | Type           | Value                                       |
|--------------------|----------------|---------------------------------------------|
| `reputation_score` | UTF-8 string   | Decimal integer, ASCII-encoded, `0..=100`. |

#### Validation rules

A consumer reading `reputation_score` MUST:

1. Reject the value if it parses to an integer outside `0..=100`.
2. Reject the value if it contains leading zeros (other than the
   single character `"0"`), trailing whitespace, sign characters,
   or non-ASCII-decimal codepoints.
3. Treat an empty string OR an absent record as "no signal" — NOT
   as a low score. The consumer's policy decides what "no signal"
   means; the ENSIP refuses to imply a default for the consumer.

A publisher writing `reputation_score` MUST:

1. Compute the score deterministically from a documented audit
   trail. The trail SHOULD be checkpoint-pinned in a sibling
   text record (e.g. `audit_root`) so a verifier can re-derive
   the score offline.
2. Emit the value with no padding, no `0x` prefix, no decimal
   point — ASCII decimal of an integer in `0..=100`.

### Optional sibling text records

| Key                       | Type    | Value                                                     |
|---------------------------|---------|-----------------------------------------------------------|
| `reputation_score_method` | UTF-8   | URI naming the interpretation contract for the score.    |
| `reputation_score_proof`  | UTF-8   | URI to a proof artefact (signed envelope, JSON, IPFS CID). |
| `reputation_score_updated` | RFC-3339 | UTC timestamp the record was last refreshed.            |

`reputation_score_method` SHOULD point to a versioned spec — for
example `https://docs.sbo3l.dev/spec/reputation/v1` or a registered
ENSIP-X identifier — so two publishers with conflicting
methodologies can coexist on the same name without overwriting each
other. The consumer chooses which methodology to honour.

`reputation_score_proof` SHOULD point to a JSON envelope of the form:

```json
{
  "schema": "<implementation-defined>",
  "score": 87,
  "computed_at": "2026-05-02T12:00:00Z",
  "audit_chain_head": "0x...",
  "signature": "0x..."
}
```

The signature pins the score to the audit chain head, so an
auditor can re-derive without trusting the publisher.

### CCIP-Read compatibility

The convention is fully compatible with ENSIP-25 / EIP-3668
CCIP-Read. A name whose resolver is an `OffchainResolver` reverts
with `OffchainLookup` for `text(node, "reputation_score")` and the
gateway answers per the ENSIP-25 protocol. This is the recommended
update path for high-frequency reputation: writing
`setText("reputation_score", ...)` per update on mainnet costs gas
linear in update frequency, while CCIP-Read pushes the cost into
gateway operation.

### Cross-chain aggregation

When the same actor is attested on multiple chains
(per a future cross-chain identity ENSIP, e.g. via signed
attestations or L2 ENS deployments), consumers MAY aggregate
per-chain `reputation_score` values into a single weighted score.
The aggregation algorithm is consumer-defined and out of scope for
this ENSIP, but a reference implementation lives at
[`crates/sbo3l-policy/src/cross_chain_reputation.rs`](../../crates/sbo3l-policy/src/cross_chain_reputation.rs)
(T-3-9). The reference parameterises over chain prominence and
recency — both surfaced as overridable inputs so consumers can
honour their own policy.

## Rationale

### Why integer 0..=100

- Reads cleanly in human UI without decoder libraries.
- Bounded so consumers can compare across implementations without
  worrying about scale drift.
- Zero ambiguity at the wire-format boundary.

Floating-point ratios were considered and rejected: parsing
"`0.870`" out of `viem.getEnsText` is awkward, and any consumer
threshold in policy code is easier expressed as integer cutoffs
(`reputation < 60 → refuse delegation`).

### Why "no signal" is not a default

A fresh agent with no audit history has no reputation signal.
Neither MIN nor MAX is right by default — the consumer's policy
decides. (SBO3L's reference scoring returns MAX as a usability
default for the v2 single-chain aggregator, but this is a *scoring*
choice, not a wire-format choice.)

### Why the methodology is a sibling record

Multiple publishers will compute reputation differently. Without
the methodology pointer, two competing implementations both
writing `reputation_score` on the same name would silently
overwrite each other. With the pointer, each publisher writes its
own methodology's record namespace and consumers honour whichever
methodology they trust.

This is the same pattern DNS uses with multiple `TXT` records each
prefixed by an interpretation domain (`v=DKIM1`, `v=SPF1`, ...).

### Why this isn't ERC-8004

ERC-8004 specifies an on-chain registry contract for agent
reputation. That's the right substrate when consumers want
gas-cheap on-chain *checks* (e.g. a smart contract gating an
action by reputation threshold). This ENSIP is the right
substrate when consumers want *human-readable, off-chain checks*
without committing to a specific registry contract per chain.

The two compose. An ERC-8004 registry MAY mirror its score into
the agent's ENS `reputation_score` for off-chain readers;
conversely, an ENSIP-conformant publisher MAY anchor its score
into an ERC-8004 registry for on-chain consumers.

## Test cases

### Valid scores

| Input string | Expected acceptance |
|--------------|---------------------|
| `"0"`        | accept              |
| `"1"`        | accept              |
| `"42"`       | accept              |
| `"100"`      | accept              |

### Rejected values

| Input string  | Reason for rejection                |
|---------------|-------------------------------------|
| `""`          | Empty → "no signal", not zero       |
| `"00"`        | Leading zeros forbidden             |
| `"01"`        | Leading zeros forbidden             |
| `"-1"`        | Sign character forbidden            |
| `"+1"`        | Sign character forbidden            |
| `"101"`       | Out of `0..=100` range              |
| `"1.0"`       | Decimal point forbidden             |
| `" 50"` / `"50 "` | Whitespace forbidden            |
| `"5e1"`       | Scientific notation forbidden       |
| `"50abc"`     | Non-decimal codepoints forbidden    |
| `"五十"`      | Non-ASCII codepoints forbidden      |

### Methodology coexistence

Two publishers (Alice using methodology `v1`, Bob using `v2`) can
coexist on the same name — each writes its own
`reputation_score_method`-pointed namespace, and consumers honour
whichever methodology they trust. The base `reputation_score` key
is overwritten by whichever publisher last set it; consumers
SHOULD prefer the methodology-specific record over the base when
the methodology is one they trust, and treat the base as a
"last-writer" signal otherwise.

## Reference implementation

- **Score publisher (Rust):**
  [`crates/sbo3l-identity/src/reputation_publisher.rs`](../../crates/sbo3l-identity/src/reputation_publisher.rs).
  Pure function from audit events to `setText` calldata for
  `sbo3l:reputation_score`. 14 unit tests pinning the wire format.
  CLI: `sbo3l agent reputation-publish --fqdn <name> --events <file>`.

- **Cross-chain aggregator (Rust):**
  [`crates/sbo3l-policy/src/cross_chain_reputation.rs`](../../crates/sbo3l-policy/src/cross_chain_reputation.rs).
  Pure function aggregating per-chain `reputation_score` values
  with caller-tunable chain prominence + recency weights. 17 unit
  tests including a synthetic 3-chain fleet.

- **Live evidence:**
  [`docs/proof/ens-narrative.md`](ens-narrative.md) walks the
  `sbo3l:reputation_score` record on Sepolia subnames behind the
  deployed `OffchainResolver`
  (`0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3`).

The reference implementation uses the prefix `sbo3l:` to namespace
SBO3L-specific records, but this ENSIP standardises the bare key
`reputation_score` for cross-publisher portability. SBO3L will
mirror its score into both `sbo3l:reputation_score` (private
namespace) and `reputation_score` (public namespace) once this
ENSIP lands.

## Backwards compatibility

ENS text records are an open namespace. This ENSIP standardises a
key name without affecting any other text record. Existing
records on `reputation_score`-shaped names (if any pre-exist) are
preserved unchanged; consumers MUST validate per the rules above
and reject malformed values rather than coerce them.

## Security considerations

- **Tampering resistance:** the score is a *commitment*, not a
  proof. A consumer that wants tamper-resistant reputation MUST
  also fetch and verify `reputation_score_proof`. The convention
  here is the wire format; the trust model is the consumer's.
- **Methodology spoofing:** without the methodology pointer, a
  malicious publisher could attempt to score-attack a victim by
  overwriting the base `reputation_score`. The pointer mitigates
  this by namespacing the score per methodology; consumers
  honouring a specific methodology are immune.
- **Stale scores:** if `reputation_score_updated` is absent, a
  consumer cannot tell a fresh score from a stale one. Consumers
  SHOULD treat absence of the freshness pointer as itself a
  signal — a publisher unwilling to commit to a freshness window
  is publishing a weaker reputation claim.

## Submission path

After ETHGlobal Open Agents 2026 closes, this draft will be:

1. Cleaned up against this ENSIP's reviewer feedback.
2. Opened as an ENSIP PR at
   [https://github.com/ensdomains/ensips](https://github.com/ensdomains/ensips).
3. Cross-referenced from the ERC-8004 reputation thread so the
   two efforts compose rather than collide.

The reference implementation's full test suite (publisher + cross-
chain aggregator) is the editorial-grade reproducibility check the
ENSIP author commits to maintaining post-merge.
