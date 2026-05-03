# Universal Router upstream submission — judge evidence

> **What this proves:** SBO3L's per-command policy-gating pattern
> for Universal Router multicalls is now a community-contributed
> reference example open at the canonical Uniswap Universal Router
> repo. Judge-clickable upstream PR.

## Upstream PR

**https://github.com/Uniswap/universal-router/pull/477**

- Title: `examples: policy-guarded swap pattern for autonomous agents (SBO3L reference)`
- Files added (in [`B2JK-Industry/universal-router` fork branch `examples-policy-guarded-swap`](https://github.com/B2JK-Industry/universal-router/tree/examples-policy-guarded-swap)):
  - `examples/policy-guarded-swap/README.md` — pattern explanation + executor_evidence JSON shape + counterfactual analysis
  - `examples/policy-guarded-swap/PolicyGuardedRouter.sol` — illustrative on-chain wrapper (~200 LOC)
- Opened: 2026-05-03 by SBO3L (B2JK-Industry) contributor

## What the example contributes

A canonical reference for the **per-command policy-gating pattern**:

1. **Decode** the Universal Router multicall into a sequence of
   typed commands (`PERMIT2_PERMIT`, `V3_SWAP_EXACT_IN`, `SWEEP`,
   `UNWRAP_WETH`, ...).
2. **Evaluate** each command independently against the policy
   engine. Each command carries its own parameters (token, amount,
   recipient) the engine inspects.
3. **Abort on first deny.** The first command that denies aborts
   the *whole* multicall — later commands aren't evaluated, no tx
   is broadcast.
4. **Capture evidence.** `executor_evidence` JSON records every
   command + its decision + the abort index, so an auditor can
   replay the decision.

Counterfactual: a naive "swap-only gate" that approves a multicall
based on the V3_SWAP leg alone misses an appended `SWEEP →
0xevil`. The per-command pattern prevents this attack.

## Why upstream

Most autonomous-agent / agentic DeFi implementations today either
sign opaque multicalls (single-shot rubber-stamp), implement
bespoke pre-flight checks (each project re-invents the boundary),
or skip policy entirely (agent has direct authority). A shared
pattern + reference impl in the Universal Router repo means
agentic apps can ship a per-command policy boundary without
reinventing the decoder + evidence shape.

The PR is intentionally framed as **community-contributed reference
documentation**, not a core Universal Router change. If
maintainers prefer this content lives elsewhere, the offer to
relocate is in the PR body.

## Reference implementation in SBO3L repo

The full Rust implementation lives in this repo at
[`crates/sbo3l-execution/src/uniswap_router.rs`](../../crates/sbo3l-execution/src/uniswap_router.rs).
Key shape:

- Decode the `(commands bytes, inputs bytes[])` pair into a
  `Vec<UniversalRouterCommand>` enum.
- Call the policy engine per-command via the `GuardedExecutor`
  trait (shared with `UniswapExecutor` for single-leg swaps).
- Evidence emitted as canonical JSON, signed Ed25519, anchored to
  the agent's audit chain (commitment in the ENS `audit_root`
  text record per [ENSIP-26](ensip-upstream-pr.md)).

## Composes with ENSIP-26

The ENSIP-26 upstream submission ([ensdomains/ensips#71](https://github.com/ensdomains/ensips/pull/71))
specifies the `audit_root` text record where the agent's audit
chain commitment lives. The Universal Router policy-guarded
pattern's evidence is what gets hashed into that audit_root —
a clean composition: ENS gives the namespace + commitment;
Universal Router gives the multi-step DeFi surface; the
per-command pattern is the bridge.

## SBO3L repository pointer

- **SBO3L repo:** https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026
- **Upstream Uniswap PR:** https://github.com/Uniswap/universal-router/pull/477
- **Companion ENSIP-26 PR:** https://github.com/ensdomains/ensips/pull/71

Submission narrative Uniswap entry will reference all three.
