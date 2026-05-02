---
title: "SBO3L → Uniswap Best API"
audience: "Uniswap bounty judges (Best API track)"
source_file: docs/submission/bounty-uniswap.md
---

# SBO3L → Uniswap Best API

> **Audience:** Uniswap bounty judges (Best API track).
> **Length:** ~500 words.

## Hero claim

**A swap executed via the Uniswap API today is opaque. SBO3L makes the audit trail cryptographic — same Uniswap API, every call gated, signed, and bound to a re-derivable policy decision.**

## Why this bounty

"Best API" rewards integrations that surface Uniswap's strengths in a way that wouldn't exist otherwise. Our argument: an autonomous agent making a swap should produce *the same on-chain effect* as a human-driven swap, plus *a portable cryptographic proof* of why that swap was authorised. Today the audit trail for an agent-driven swap is "whatever the agent code happened to write" — i.e. usually nothing verifiable. With SBO3L's `UniswapExecutor` as a `GuardedExecutor`, every swap intent runs through a deterministic policy boundary before the swap is constructed; slippage, MEV-protection, token-allowlist, and value-cap are all expressed as policy rules — not as bot logic. The decision is signed; the audit row links that decision to the eventual on-chain `tx_hash` via the Passport capsule.

The Uniswap API surface we exercise is intentionally vanilla: QuoterV2 for quotes, Universal Router for swap construction, and the standard SwapRouter02 for direct swaps. **No custom contracts, no v4 hooks competing for sponsor attention, no rewrites of Uniswap's own primitives.** Our value-add is the cryptographic envelope around the Uniswap call, not the call itself.

## Technical depth

### Sepolia QuoterV2 — live quote evidence

`UniswapExecutor::live_from_env()` calls QuoterV2 (`0xEd1f6473345F45b75F8179591dd5bA1888cf2FB3`) on Sepolia for every swap intent. The quote evidence (`quote_source: uniswap-v3-quoter-sepolia-…`, real `sqrt_price_x96_after`, freshness timestamp, route `WETH → USDC 0x1c7D4B19…`) is captured into the Passport capsule. Off-by-one or stale quotes → policy `protocol.deny_quote_stale` rejection.

### Universal Router with per-step policy gates (T-5-2 ✅, [PR #171](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/171))

Every Universal Router command (`V3_SWAP_EXACT_IN`, `WRAP_ETH`, `PERMIT2_PERMIT`, etc.) is gated by an *independent* policy decision before encoding into the calldata stream. Slippage, recipient allowlist, and value-cap rules apply per step rather than per-bundle — so a multi-hop swap can have different slippage tolerance per hop, and a recipient deny on hop 3 doesn't already-spent hops 1 and 2.

### Smart Wallet integration with per-call policy gates (T-5-3 ✅, [PR #183](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/183))

The agent acts as the Smart Account owner. Each call inside a Smart Wallet batch carries its own SBO3L PolicyReceipt and audit-event linkage. The capsule contains the full per-call decision tree, not just the outer batch result. Tampering with one inner call's `live_evidence` invalidates the strict-hash verifier output for the whole capsule.

### MEV protection in policy (T-5-4 in flight, [PR #179](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/179))

Policy rules express MEV-safety primitives directly: `slippage_bps` ≤ `policy.max_slippage`, `priority_fee` bounded, `quote.freshness` ≤ `policy.max_quote_age`, `recipient` in `policy.allowed_recipients`. The audit row records exactly which rule fired; an auditor can reconstruct the bounded swap path *without trusting our daemon being online*.

## Live verification

- **Sepolia QuoterV2 live quote:** `cargo install sbo3l-cli --version 1.2.0 && SBO3L_UNISWAP_RPC_URL=… sbo3l passport run swap-aprp.json --executor uniswap --mode live --quote-only --out /tmp/capsule.json && sbo3l passport verify --strict --path /tmp/capsule.json` → PASSED with quote evidence in capsule
- **Real Sepolia swap with `tx_hash` in capsule** (T-5-5, gated on T-5-1 #165) — capsule's `execution.live_evidence.tx_hash` is the canonical proof; Etherscan link will be captured into `demo-scripts/artifacts/uniswap-real-swap-capsule.json` (artifact dir is gitignored — file appears at run time after `bash demo-scripts/sponsors/uniswap-real-swap.sh` against funded wallet)
- **Examples:** `examples/uniswap-agent-{ts,py}/` (T-5-6, [PR #166](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/166)) — TS + Py demo
- **Same capsule re-verified in browser:** drag into https://sbo3l.dev/proof — WASM verifier runs offline checks; tampering with `tx_hash` → `capsule.live_evidence_mismatch`

## Sponsor-specific value prop

The cryptographic envelope is what's *new*. The Uniswap call inside it is intentionally identical to what a human-driven swap looks like. Auditors, regulators, and operators can re-derive what was authorised, by whom, against which policy — across days, across teams, across systems — without trusting any single party. **That's the difference between an agent-driven swap and an opaque swap.** Same Uniswap API; cryptographic audit trail.
