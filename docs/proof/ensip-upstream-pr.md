# ENSIP-26 upstream submission — judge evidence

> **What this proves:** SBO3L's agent-identity records convention
> isn't just a SBO3L-internal tag system. The ENSIP-26 spec is in
> review at the canonical ENS standards repo, where every other
> ENSIP (1-25) lives. Judge-clickable upstream PR.

## Upstream PR

**https://github.com/ensdomains/ensips/pull/71**

- Title: `ENSIP-26: Agent Identity Records for Autonomous Agent Discovery`
- File added: [`ensips/26.md`](https://github.com/ensdomains/ensips/blob/master/ensips/26.md) (363 LOC) on the [B2JK-Industry/ensips fork branch `ensip-26-agent-identity-records`](https://github.com/B2JK-Industry/ensips/tree/ensip-26-agent-identity-records)
- Status: Draft (per ENSIP review process — editors triage, then move to Review/Last Call/Final)
- Opened: 2026-05-03 by SBO3L (B2JK-Industry) contributor
- Track: ENS contact Dhaiwat confirmed late entrants accepted ("you still have time")

## What ENSIP-26 specifies

Seven standardised text-record keys for publishing autonomous-agent
identity on ENS, so any ENS-aware client can read agent metadata
with zero bespoke decoders:

```
agent_id, endpoint, pubkey_ed25519, policy_hash,
audit_root, capability, reputation_score
```

Each key has a normative format + interpretation. Fully opt-in
(names that don't claim to be agents are unaffected). Composes
with ENSIP-25 (verification) and ERC-8004 (on-chain registry) by
design — different consumer shapes, no overlap.

## Why this is a real ENSIP, not vanity

1. **Working reference implementation.** SBO3L's mainnet apex
   `sbo3lagent.eth` resolves all seven canonical records today.
   Verifiable from a public RPC + `cast`:

   ```bash
   RESOLVER=0xF29100983E058B709F3D539b0c765937B804AC15
   NODE=$(cast namehash sbo3lagent.eth)
   for KEY in agent_id endpoint pubkey_ed25519 policy_hash audit_root capability reputation_score; do
     printf '%s = ' "$KEY"
     cast call "$RESOLVER" "text(bytes32,string)(string)" "$NODE" "$KEY" \
       --rpc-url https://ethereum-rpc.publicnode.com
   done
   ```

2. **CCIP-Read demonstrated end-to-end.** Sepolia subname
   `research-agent.sbo3lagent.eth` resolves through the deployed
   OffchainResolver at
   [`0x87e99508C222c6E419734CACbb6781b8d282b1F6`](https://sepolia.etherscan.io/address/0x87e99508C222c6E419734CACbb6781b8d282b1F6).
   viem.getEnsText returns `"research-agent-01"` end-to-end
   through the CCIP-Read flow.

3. **60-agent constellation manifest** at
   [`docs/proof/ens-fleet-agents-60-2026-05-01.json`](ens-fleet-agents-60-2026-05-01.json)
   shows the convention scaling beyond a single demo agent.

4. **Composes with ENSIP-25** (which Phase 2 of ENS shipped, also
   covering AI agents) — ENSIP-25 verifies an ENS name's
   association with an ERC-8004 registry entry; ENSIP-26 specifies
   the metadata the ENS-name-bound representation carries. No
   redundancy.

## What unblocks downstream

- **14+ agent framework adapters** (LangChain, LangGraph, CrewAI,
  AutoGen, ElizaOS, LlamaIndex, Vercel AI, OpenAI Assistants,
  Anthropic, ...) can ship `getAgentIdentity(ensName)` without
  bespoke decoders per platform.
- **ERC-8004 reference impls** can mirror their agent metadata
  into ENS records, giving off-chain consumers cheap reads.
- **Cross-agent verification** (agent A reading agent B's identity
  to refuse-or-accept delegation) becomes a one-liner:
  `viem.getEnsText(name, "pubkey_ed25519") + verify(challenge,
  signature)`.

## Discussion thread

Linked from the upstream PR body. Reviewers Dhaiwat and Simon
(`ses.eth`) are the ENS contacts SBO3L coordinated with on the
broader ENS bounty submission — they're already aware of the
reference implementation.

## SBO3L repository pointer

The upstream PR cites SBO3L's repo as the reference impl; this
file in the SBO3L repo cites the upstream PR back for symmetry
and judge navigation:

- **SBO3L repo:** https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026
- **Upstream PR:** https://github.com/ensdomains/ensips/pull/71

Submission narrative ENS Most Creative entry will reference both.
