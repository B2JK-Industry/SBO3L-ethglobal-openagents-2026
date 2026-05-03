# ENSIP-26 community follow-up — judge evidence

> **What this proves:** SBO3L isn't just opening upstream PRs and
> waiting silent — we converted the [ENSIP-26 PR review](https://github.com/ensdomains/ensips/pull/71)
> into an active discussion thread by opening a sister issue with
> 5 specific design questions for the ENS community.

## Upstream issue

**https://github.com/ensdomains/ensips/issues/72**

- Title: `ENSIP-26 (Agent Identity Records) — discussion: 5 design questions for community feedback`
- Cross-linked to [ENSIP-26 PR #71](https://github.com/ensdomains/ensips/pull/71)
- Opened: 2026-05-03 by SBO3L (B2JK-Industry) contributor

## What the issue asks

5 specific design questions for ENS-community feedback. Each is
phrased as "X vs Y, here's the trade-off, which way does the
spec go" — none is "is this a good idea" (that's the PR's job).

| # | Question | Why it matters |
|---|---|---|
| Q1 | Bare keys (`agent_id`, ...) vs `agent.*` namespace prefix | Consumer ergonomics vs collision safety |
| Q2 | All-7 vs required-core + optional-extensions | Partial-agent identity semantics |
| Q3 | Fixed Ed25519 (`pubkey_ed25519`) vs polymorphic family (`pubkey_<scheme>`) | One scheme = simpler verifiers; polymorphic = forward compat |
| Q4 | JSON array `capability` (open tags) vs comma-separated + canonical registry | Maintenance burden vs tag fragmentation |
| Q5 | `reputation_score` in ENSIP-26 vs split into sibling ENSIP | Spec scope: identity-only vs identity+reputation |

Each question has the trade-off laid out + a concrete ask. Reviewers can React 👍 or comment a stance.

## Live verifiable example in the issue body

The issue includes a paste-runnable verification command for
`sbo3lagent.eth` mainnet apex:

```bash
RESOLVER=0xF29100983E058B709F3D539b0c765937B804AC15
NODE=$(cast namehash sbo3lagent.eth)
for KEY in agent_id endpoint pubkey_ed25519 policy_hash audit_root capability reputation_score; do
  printf '%s = ' "$KEY"
  cast call "$RESOLVER" "text(bytes32,string)(string)" "$NODE" "$KEY" \
    --rpc-url https://ethereum-rpc.publicnode.com
done
```

Plus a viem-style CCIP-Read example for the Sepolia subname:

```bash
viem.getEnsText({
  name:   'research-agent.sbo3lagent.eth',
  key:    'agent_id',
  chain:  sepolia,
})
// → "research-agent-01"
```

This converts the issue from "abstract design discussion" to
"design discussion grounded in a live reference impl reviewers
can probe before forming opinions."

## Cross-track linking

The issue body explicitly cites the two companion upstream PRs:

- [Uniswap/universal-router#477](https://github.com/Uniswap/universal-router/pull/477)
  (per-command policy gating — uses ENSIP-26 `audit_root` as
  the on-chain audit-anchor commitment)
- [KeeperHub/cli#57](https://github.com/KeeperHub/cli/pull/57)
  (workflow envelope `sbo3l_*` fields — `sbo3l_audit_root` is
  the citation back to the ENSIP-26 text record)

This tells reviewers: "ENSIP-26 isn't an isolated proposal;
it's the namespace the agentic-DeFi stack we're building cites
back to."

## Why upstream issue (vs PR comments)

The PR thread is good for **line-level technical edits** ("rename
this field," "tighten this validation rule"). The issue is for
**higher-level design questions** that need community input
before the spec can lock in.

Per memory `competitor_intel_2026-05-03.md`:
> ENS team accepts late entrants — Dhaiwat: "you still have time."

A live discussion thread converts "we opened a PR" (silent) into
"we're driving a community conversation" (visible activity).
Higher merge probability + better judge perception.

## SBO3L repository pointer

- **SBO3L repo:** https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026
- **ENSIP-26 PR:** https://github.com/ensdomains/ensips/pull/71
- **ENSIP-26 follow-up issue:** https://github.com/ensdomains/ensips/issues/72
- **Companion ENSIP evidence doc:** [`docs/proof/ensip-upstream-pr.md`](ensip-upstream-pr.md)

Submission narrative ENS Most Creative entry will reference the
issue alongside the PR.
