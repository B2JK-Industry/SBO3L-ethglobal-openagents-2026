---
title: "SBO3L → ENS AI Agents"
audience: "ENS bounty judges (Best ENS Integration for AI Agents track)"
source_file: docs/submission/bounty-ens-ai-agents.md
---

# SBO3L → ENS AI Agents

> **Audience:** ENS bounty judges (Best ENS Integration for AI Agents track).
> **Length:** ~500 words.

## Hero claim

**Identity (ENS) + dynamic state (ENSIP-25 CCIP-Read) + global registry (ERC-8004) — composed into a single agent trust profile any other agent can verify.** Three ENS-ecosystem standards, one cryptographically-coherent agent identity surface.

## Why this bounty

The "AI Agents" track rewards depth in ENS-as-agent-infrastructure, not just naming. Our submission stacks three independent ENS-aligned standards into a single trust profile:

1. **ENS text records (mainnet)** — static identity (`agent_id`, `endpoint`, `policy_hash`, `audit_root`, `proof_uri`)
2. **ENSIP-25 CCIP-Read gateway** — dynamic state (current `audit_root`, fresh `capability`, computed `reputation`) without burning gas on every update
3. **ERC-8004 Identity Registry** — global agent identity anchor (Sepolia LIVE at `0x600c10dE2fd5BB8f3F47cd356Bcb80289845Db37`; mainnet deploy gated on operator wallet PK + ~$50 gas), linking the ENS subname to a verifiable canonical pubkey

Each layer is independently useful. Stacked, they answer every question an autonomous agent needs to ask before trusting another autonomous agent: *who are you, what are you allowed to do, what's your current reputation, and is your identity globally verifiable?*

## Technical depth

### CCIP-Read gateway (T-4-1, end-to-end live)

ENSIP-25 / EIP-3668 off-chain resolution. SBO3L's gateway at https://sbo3l-ccip.vercel.app is consumed automatically by `viem.getEnsText`, `ethers.js`, and any ENSIP-10-aware client. **No SBO3L-specific code on the client side.** A client resolving `research-agent.sbo3lagent.eth` text record `sbo3l:reputation` follows the OffchainResolver redirect to our gateway, which computes the current 4-criteria reputation score from the live audit chain and returns a signed envelope verified against the OffchainResolver contract.

- **OffchainResolver contract:** [`crates/sbo3l-identity/contracts/OffchainResolver.sol`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/crates/sbo3l-identity/contracts/OffchainResolver.sol) — Foundry test suite at [`crates/sbo3l-identity/contracts/test/OffchainResolver.t.sol`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/crates/sbo3l-identity/contracts/test/OffchainResolver.t.sol); deploy via [`scripts/deploy-offchain-resolver.sh`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/scripts/deploy-offchain-resolver.sh) — Sepolia deployed
- **Gateway endpoint:** `GET /api/{sender}/{data}.json` (Next.js dynamic route)
- **Rust client decoder:** `crates/sbo3l-identity` — ENSIP-25 wire-format decoder for clients that don't use viem/ethers
- **Uptime probe:** [`.github/workflows/ccip-gateway-uptime.yml`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/.github/workflows/ccip-gateway-uptime.yml)

### ERC-8004 Identity Registry

T-4-2 — agents register their `agent_id` + canonical pubkey in the ERC-8004 Identity Registry, anchoring the ENS subname to a global identity. **Live Sepolia deployment** at `0x600c10dE2fd5BB8f3F47cd356Bcb80289845Db37` (PR #358). Mainnet deployment scoped for the post-submission window (gated on operator wallet PK + ~$50 mainnet gas — explicitly NOT a hackathon-window claim). Calldata builders + dry-run path live in `crates/sbo3l-identity/src/erc8004.rs`.

### Cross-agent reputation (T-4-3 ✅ shipped at v1.2.0)

`sbo3l:reputation` text record computed from the audit chain via 4-criteria scoring (success rate, deny rate, recency, consensus-with-peers). The reputation is dynamic — the live record reflects the agent's current state, not an init-time snapshot. CCIP-Read makes this practical without per-update gas.

## Live verification

- **CCIP gateway:** https://sbo3l-ccip.vercel.app/ + smoke fail-mode `/api/0xdeadbeef/0x12345678.json` returns 400 (correct rejection of invalid sender) per [`docs/submission/url-evidence.md`](url-evidence.md)
- **CLI resolve dynamic record:** `sbo3l agent verify-ens research-agent.sbo3lagent.eth --rpc-url https://ethereum-rpc.publicnode.com` — gateway hit, signed response verified
- **Mainnet apex byte-match:** `sbo3l-identity` CI test asserts the on-chain `policy_hash` byte-matches the offline fixture (no drift)
- **Reputation 4-criteria scoring tests:** `crates/sbo3l-policy/src/reputation.rs` + integration test fleet (PR #126 ✅)

## Sponsor-specific value prop

Most "agent identity" projects pick *one* layer of the ENS-ecosystem stack. We pick three, and we make them compose. A judge who wants to know "is this agent trustworthy?" gets back: ENS text records (static commitments), CCIP-Read (current state), ERC-8004 (global identity anchor) — all verified cryptographically end-to-end, all consumable by any standard ENS client without SBO3L-specific tooling. **This is what an agent-grade ENS integration looks like at v1.**
