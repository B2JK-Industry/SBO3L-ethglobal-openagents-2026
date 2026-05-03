# KeeperHub protocol upstream submission — judge evidence

> **What this proves:** SBO3L's IP-1 envelope spec (the
> trust-receipt fields shipped in `sbo3l-keeperhub-adapter`) is
> now an open proposal at the canonical KeeperHub CLI repo.
> Judge-clickable upstream PR.

## Upstream PR

**https://github.com/KeeperHub/cli/pull/57**

- Title: `docs: propose policy-receipt envelope fields for upstream-proof workflow submits (SBO3L reference)`
- File added: `docs/proposals/upstream-policy-receipts.md` (155 LOC) on the [B2JK-Industry/cli fork branch `docs-upstream-policy-receipts`](https://github.com/B2JK-Industry/cli/tree/docs-upstream-policy-receipts)
- Opened: 2026-05-03 by SBO3L (B2JK-Industry) contributor

## What the proposal contributes

A **protocol-level** ask (different from the integration-level
asks in KeeperHub/cli issues #52-#56): canonicalise an optional
`sbo3l_*`-prefixed envelope on workflow webhook submit so an
upstream caller can attach a signed policy receipt + audit-anchor
commitment to every workflow execution.

The six fields:

```
sbo3l_policy_hash, sbo3l_audit_root, sbo3l_pubkey_ed25519,
sbo3l_receipt, sbo3l_capsule_uri, sbo3l_evidence_chain_position
```

Each independently meaningful, all optional, all prefix-namespaced
so KH's core surface is unchanged. Vendor-generalisable
(`langchain_*`, `crewai_*`, ...) — the prefix itself is the
vendor token.

## Why upstream

KeeperHub workflow submissions are currently **opaque** at the
trust layer. Sponsors integrating with KeeperHub either trust
agents unconditionally (single-shot rubber-stamp), maintain
bespoke pre-flight envelopes (every project diverges), or refuse
autonomous-agent submissions entirely (safe but limiting).

A standardised optional envelope shape means agentic platforms
can ship KeeperHub adapters with one consistent trust surface
without re-deriving it per-vendor. The reference implementation
already exists: [`sbo3l-keeperhub-adapter`](https://crates.io/crates/sbo3l-keeperhub-adapter)
v1.2.0 attaches these fields when submitting workflows on behalf
of SBO3L agents.

## Composition with the existing 5-issue track

| KH issue | What it asks | Companion SBO3L PR |
|---|---|---|
| [#52](https://github.com/KeeperHub/cli/issues/52) HTTP error code catalog | Operational ergonomics | [PR #402](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/402) |
| [#53](https://github.com/KeeperHub/cli/issues/53) Mock fixture suite | CI ergonomics | [PR #403](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/403) |
| [#54](https://github.com/KeeperHub/cli/issues/54) Timeout SLO publication | Operational ergonomics | [PR #404](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/404) |
| [#55](https://github.com/KeeperHub/cli/issues/55) Schema versioning | Forward compatibility | [PR #405](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/405) |
| [#56](https://github.com/KeeperHub/cli/issues/56) Payload size + 413 envelope | Operational ergonomics | [PR #406](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/406) |
| **[#57 (this proposal)](https://github.com/KeeperHub/cli/pull/57) Policy-receipt envelope** | **Protocol** | (full adapter ships fields today on crates.io v1.2.0) |

The issues + reference PRs are the **integration-level** track —
showing what consumer-side changes the adapter would land once
each KH issue ships. This proposal is the **protocol-level** ask
sitting alongside, complementary not redundant.

## Open questions in the upstream PR

- Prefix naming: `sbo3l_*` (vendor-generalisable) vs
  `kh-recommended-*` (KH-canonicalised).
- Receipt signature scheme: specified (Ed25519) or
  implementation-specific.
- Server-side validation behaviour: stricter vs agnostic.
- Composition with the proposed Webhook Schema Versioning ([#55](https://github.com/KeeperHub/cli/issues/55)).

## Comms posture (memory `competitor_intel_2026-05-03.md`)

Luca (KH team) explicitly said "Happy final sprint" → **going
dark for engineering Qs**. This PR is open-loop public evidence —
no expectation of pre-merge feedback. The proposal sits in their
queue post-sprint; the upstream URL is what matters for the
bounty submission.

## SBO3L repository pointers

- **SBO3L repo:** https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026
- **`sbo3l-keeperhub-adapter` on crates.io:** https://crates.io/crates/sbo3l-keeperhub-adapter (v1.2.0 LIVE)
- **Upstream KH protocol PR:** https://github.com/KeeperHub/cli/pull/57
- **Companion KH 5-issue track:** https://github.com/KeeperHub/cli/issues/52..56

Submission narrative KH Best Use entry will reference all four.
