# Mainnet OffchainResolver deploy — decision (R12 P5)

**Decision:** **SKIP** the mainnet OffchainResolver deploy for the
hackathon submission. The Sepolia deploy
([`0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3`](https://sepolia.etherscan.io/address/0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3))
provides demo-equivalent value at zero mainnet-gas cost and zero
operational risk to the live mainnet `sbo3lagent.eth` apex.
**Status:** documented + signed off; revisitable post-hackathon.
**Decision-maker:** Daniel (per R12 P5 spec — "If NOT (recommended):
document why Sepolia OffchainResolver provides identical demo
value").

## Why skip

### 1. Sepolia OffchainResolver is fully demonstrative

Every claim the mainnet deploy would prove is already provable on
Sepolia today:

- **Contract is deployed and verified.** Etherscan-verified at
  `0x7c6913…aCA8c3`; bytecode + ABI public.
- **Gateway responds end-to-end.** `sbo3l-ccip.vercel.app` serves
  signed responses to ENSIP-25 / EIP-3668 CCIP-Read clients.
- **Fuzz suite passes 10K runs.** `OffchainResolver.invariant.t.sol`
  has 11 fuzz tests at 10 000 runs each; same suite runs on the
  same code mainnet would deploy.
- **viem E2E demo works.** `examples/t-4-1-viem-e2e/` runs against
  the Sepolia contract with no SBO3L-specific decoder; a judge's
  laptop can reproduce the resolution flow in 30 seconds.

Mainnet's only marginal contribution would be **psychological
confidence** ("this works on mainnet too"). The technical claim is
the same.

### 2. Mainnet migration risk on `sbo3lagent.eth` is non-trivial

The mainnet apex `sbo3lagent.eth` currently uses the canonical
PublicResolver and serves the five `sbo3l:*` records on chain
directly. Migrating to a fresh `OffchainResolver` deploy means:

1. Deploy `OffchainResolver` to mainnet (~$5 gas at 50 gwei,
   one-time).
2. Re-issue all five records via the gateway's signed-response
   path (gateway must be ready + key pinned at deploy time).
3. Call `setResolver(sbo3lagent.eth, <new resolver>)` on the ENS
   Registry (~$1 gas).
4. Verify every existing consumer (the verify-ens CLI, the viem
   demo, the cross-agent protocol's `getEnsText` for the agent's
   pubkey) still resolves correctly post-migration.

**Failure modes:**

- Gateway misconfigured at swap time → all five records return
  empty until fixed → consumers break silently.
- Gateway-signing key compromised → attacker forges any record;
  recovery = redeploy + re-`setResolver` (chain operations under
  attacker pressure).
- Migration mid-judge-window → hackathon submission verification
  partially fails depending on which RPC's caches are warm.

These are recoverable risks but they're risks. The Sepolia path
has none of them — Sepolia's records are isolated from mainnet
consumers.

### 3. Cost-efficient mainnet alternative is in the spec

If we want mainnet visibility without migrating the apex, the
T-3-1 broadcast slice (#169) lets us register a *new* mainnet
subname under `sbo3lagent.eth` (e.g. `demo.sbo3lagent.eth`) and
demonstrate the registration flow live. Cost: ~$5 mainnet gas for
the `setSubnodeRecord` + `setText × N`. That's R12 P3-style
showcase-register territory, NOT this OffchainResolver migration.
The two are independent decisions.

## Conditions to revisit

Mainnet OffchainResolver deploy becomes the right call when:

1. **An external partner integrates against `sbo3lagent.eth`** and
   asks for OffchainResolver-style dynamic records (current
   on-chain records are static). Until a partner asks, the static
   path is sufficient.
2. **Daniel's reputation broadcast pipeline (#267 multi-chain) is
   ready to publish reputation_score updates faster than per-update
   `setText` is economical**. CCIP-Read is the right mechanism
   then; today the score updates are checkpoint-gated and `setText`
   is fine.
3. **An ENS or third-party reviewer raises the mainnet question
   specifically as a bounty-evaluation criterion**. So far the
   bounty narrative passes on Sepolia.

When any of those land, the migration runbook is documented in
[`docs/cli/ens-fleet-sepolia.md`](../cli/ens-fleet-sepolia.md)
under "mainnet migration" — same script, mainnet RPC, plus the
standard `SBO3L_ALLOW_MAINNET_TX=1` double-gate.

## What the bounty narrative says

The ENS Most Creative bounty submission
([`docs/submission/bounty-ens-most-creative-final.md`](../submission/bounty-ens-most-creative-final.md))
explicitly notes "mainnet broadcast for fleet agents" + "live
broadcast of dynamic records" as **honestly-scoped limitations**
— not deficiencies. The narrative argues that the hackathon-shaped
demo is best served by Sepolia's risk-free reproducibility, with
the mainnet path documented as a follow-up gated on operator
decision rather than left as an assumed step.

## Sign-off

This document is the operator-side sign-off on skipping mainnet
for the hackathon submission. Future PR that revisits the
decision should reference this file, articulate which of the three
"conditions to revisit" triggered, and update the bounty narrative
+ deploy runbook accordingly.

Signed off (R12 P5): Daniel + Dev 4, 2026-05-02.
