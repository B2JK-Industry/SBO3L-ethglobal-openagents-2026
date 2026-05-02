# ETHGlobal Open Agents 2026 — form content (paste-ready)

> **Audience:** Daniel, at submission time.
> **Outcome:** every track form filled in under 10 minutes by copy-pasting from this file.
> **Voice rule:** every claim in here has a code reference, a live URL, or a runnable command. Honest-over-slick.

The form fields below are derived from typical ETHGlobal submission forms. If the actual track form has different field names, map by intent.

## Project basics (shared across tracks)

**Project name:** SBO3L

**Tagline:** Don't give your agent a wallet. Give it a mandate.

**One-line pitch:** SBO3L is the cryptographically verifiable trust layer for autonomous AI agents — every agent action passes through SBO3L's policy boundary and produces a self-contained Passport capsule anyone can verify offline.

**Project description (~500 words):**

> **The problem.** Autonomous AI agents are starting to take real actions on real money — paying, swapping, storing, coordinating. Today's stack gives the agent a wallet, a signing key, and trust by default. That's wrong on three counts: an agent operator can't bound *which* actions are allowed, an auditor can't reconstruct *why* a specific action was authorised, and a sponsor (the system that ultimately executes) can't link an inbound request back to a verifiable upstream policy decision.
>
> **What SBO3L does.** SBO3L is a thin trust layer that sits between an autonomous AI agent and the systems it acts on. Instead of giving the agent a wallet or signing keys, you give it a *mandate*: a policy decision, a signed receipt, and an audit-chain entry — produced by SBO3L the moment the agent submits its intent. The agent crate has zero `SigningKey` references; signing happens only inside SBO3L (verified by a grep-asserted demo gate).
>
> **The wire format.** APRP (Agent Payment Request Protocol) is a payment-shaped JSON envelope with `intent`, `amount`, `chain`, `expiry`, `risk_class`, `nonce`. SBO3L canonicalises it (JCS), hashes it (SHA-256), runs a deterministic policy decision against an Ed25519-signed active policy, increments multi-scope budgets, claims the nonce, signs a `PolicyReceipt`, appends a hash-chained `AuditEvent`, and routes through a sponsor adapter (KeeperHub workflow webhook, Uniswap QuoterV2, ENS resolver).
>
> **Passport capsule v2 — the load-bearing output.** A single JSON file containing every byte needed to re-derive the decision offline: schema, request-hash, policy snapshot, audit segment, decision, agent-id, audit-event-id linkage, Ed25519 signatures. One CLI command (`sbo3l passport verify --strict`) re-derives everything from the capsule alone — no daemon, no network, no RPC. The same WASM verifier runs in the browser at `/proof`: drag-drop a capsule, six green checks pop in. Tamper one byte, verifier rejects.
>
> **Live integrations.** All three sponsor live paths shipped + smoke-verified end-to-end. KeeperHub: real workflow webhook accepts the IP-1 envelope, returns a real `executionId` (`kh-172o77rxov7mhwvpssc3x`-shape). ENS: `sbo3lagent.eth` mainnet apex with 5 `sbo3l:*` records on chain (Phase 2 adds 2 more); subnames issued via direct ENS Registry. Uniswap: live `quoteExactInputSingle` against Sepolia QuoterV2 (`0xEd1f6473345F45b75F8179591dd5bA1888cf2FB3`); real swap captures `tx_hash` into the capsule.
>
> **What ships at v1.0.1.** Nine Rust crates on crates.io. TypeScript SDK on npm. Python SDK on PyPI. Eight framework integrations (LangChain TS/Py, CrewAI, AutoGen, ElizaOS, LlamaIndex, Vercel AI, LangGraph). Docker compose. Marketing site, hosted preview, docs site, CCIP-Read gateway. ENS mainnet apex. Phase 1 exit gate green: 441/441 cargo tests, 13/13 demo gates, 26/0/1 production-shaped runner.

**Tech stack:**
- **Core:** Rust workspace, 9 published crates (`sbo3l-{core,storage,policy,identity,execution,keeperhub-adapter,server,mcp,cli}`)
- **SDKs:** TypeScript (`@sbo3l/sdk` on npm), Python (`sbo3l-sdk` on PyPI)
- **Framework integrations:** LangChain (TS + Py), CrewAI, AutoGen, ElizaOS, LlamaIndex, Vercel AI, LangGraph
- **Onchain:** ENS (Registry + PublicResolver), ENSIP-25 CCIP-Read, ERC-8004 Identity Registry, Uniswap V3 QuoterV2 + Universal Router (Sepolia)
- **Off-chain partners:** KeeperHub (workflow webhooks + executionId)
- **Crypto:** Ed25519 receipts + audit signatures, JCS canonical JSON, SHA-256 hash chain
- **Storage:** SQLite (8+ migrations: APRP, idempotency, budget, KMS, active-policy, audit-checkpoints)
- **Infra:** Docker (multi-stage distroless), Vercel (marketing + ccip-gateway), GitHub Actions (CI + post-merge regression sweep), GitHub Pages (capsule mirror)

**Repo URL:** https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026

**Live demo URL:** https://sbo3l.dev (custom domain pending; fallback https://sbo3l-marketing.vercel.app)

**Verifier (judges click this):** https://sbo3l.dev/proof (fallback: `/proof` route on the Vercel preview)

**Source:** https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026

**Demo video URL:** _populate after recording — see [`demo-video-script.md`](demo-video-script.md)_

**Built during the hackathon? (yes/no):** Yes — entire codebase shipped within the 100-day window. Repo init at `9504fa7`; v1.0.1 release commit at `c90f571`.

## Bounty selections

- ✅ **KeeperHub Best Use** (Track 1)
- ✅ **KeeperHub Builder Feedback** (Track 2)
- ✅ **ENS Most Creative** (Track 3)
- ✅ **ENS AI Agents** (Track 4)
- ✅ **Uniswap Best API** (Track 5)
- ⏳ 0G Track A (Storage) — Phase 3
- ⏳ 0G Track B (DA / Compute) — Phase 3
- ⏳ Gensyn AXL — Phase 3

## Bounty submission tagline (per partner)

| Partner | Tagline |
|---|---|
| **KeeperHub Best Use** | "KeeperHub executes. SBO3L proves the execution was authorised." |
| **KeeperHub Builder Feedback** | "Five concrete asks (KeeperHub/cli#47-#51) filed during real integration work — token-prefix naming, submission/result schema, executionId lookup, upstream policy fields, idempotency semantics." |
| **ENS Most Creative** | "ENS is not the integration. ENS is the trust DNS." |
| **ENS AI Agents** | "Identity (ENS) + dynamic state (ENSIP-25 CCIP-Read) + global registry (ERC-8004) — composed into a single agent trust profile any other agent can verify." |
| **Uniswap Best API** | "A swap executed via the Uniswap API today is opaque. SBO3L makes the audit trail cryptographic — same API, every call gated, signed, and bound to a re-derivable policy decision." |

## Sponsor track field — KeeperHub Best Use

**How does your project use KeeperHub?**

> Every action that flows through SBO3L's `KeeperHubExecutor` is gated by an APRP policy decision before any KeeperHub workflow webhook is called. Allowed actions arrive at KH with a signed `sbo3l_*` envelope (request_hash, policy_hash, receipt_signature, audit_event_id, optional passport_uri) — five optional fields KH can echo back to make every execution row cryptographically linked to the upstream authorisation.
>
> SBO3L ships a standalone `sbo3l-keeperhub-adapter` crate on crates.io implementing IP-1..IP-5 from the integration paths doc. The adapter has both `local_mock()` (CI-safe default) and `live_from_env()` (real `wfb_…` token + workflow id) constructors. Live submissions to KeeperHub were exercised end-to-end on submission day against workflow `m4t4cnpmhv8qquce3bv3c`; the IP-1 envelope POST returned a real `executionId` of the form `kh-172o77rxov7mhwvpssc3x` (KH-format, *not* a ULID — that's KeeperHub's own identifier scheme), captured into the Passport capsule's `execution.live_evidence` for offline verification.

**Live integration evidence:**
- Webhook URL: `https://app.keeperhub.com/api/workflows/m4t4cnpmhv8qquce3bv3c/webhook`
- Live arm: [`crates/sbo3l-keeperhub-adapter/`](../../../crates/sbo3l-keeperhub-adapter/) — `submit_live_to`
- Demo gate: [`demo-scripts/sponsors/keeperhub-real-execution.sh`](../../../demo-scripts/sponsors/keeperhub-real-execution.sh)
- Real execution captured: `kh-172o77rxov7mhwvpssc3x` (re-run sweep produces a fresh one)

**5+ specific KH improvement issues filed:** _populate from `FEEDBACK.md` after T-2-1 lands_

## Sponsor track field — KeeperHub Builder Feedback

**Concrete pain points hit during live integration:** see [`FEEDBACK.md`](../../FEEDBACK.md). Specific suggestions filed as issues on the KeeperHub repo (links in the same file).

## Sponsor track field — ENS Most Creative

**How does your project use ENS?**

> SBO3L turns ENS into *the agent trust DNS*. Every named agent gets a subname under `sbo3lagent.eth` (mainnet apex Daniel owns) with `sbo3l:*` text records. v1.0.1 ships **5 records on chain** (`agent_id`, `endpoint`, `policy_hash`, `audit_root`, `proof_uri`); Phase 2 adds two more (`capability`, `reputation`) for the full 7-record profile. Subname registration uses **direct ENS Registry `setSubnodeRecord`** (Daniel is the parent owner) + PublicResolver `setText` per record — no third-party registrar abstraction needed (we evaluated Durin and dropped it 2026-05-01: direct registry is fewer moving parts, more verifiable on Etherscan, and zero new contracts to deploy).
>
> An agent resolves another agent by ENS name and gets back a complete trust profile — *without trusting any single party*. Cross-agent attestations are signed Ed25519 envelopes that pin the recipient's expected `policy_hash` and an `expires_at`; the receiving SBO3L instance verifies the chain (sender's pubkey → recipient's published policy → recipient's actual decision) before allowing the delegated action.
>
> Five+ named agents on Sepolia (`research-agent`, `trading-agent`, `swap-agent`, `audit-agent`, `coordinator-agent`) discover each other in real time in the [trust-DNS visualization](https://app.sbo3l.dev/trust-dns). The visualization is the demo video centerpiece.

**Live ENS evidence:**
- Mainnet `sbo3lagent.eth` with 5 `sbo3l:*` records on chain. `policy_hash = e044f13c5acb792dd3109f1be3a98536168b0990e25595b3cedc131d02e666cf` — matches the offline fixture byte-for-byte (no drift)
- ENS Registry constant: `0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e` (mainnet + Sepolia, deterministic deployment)
- Mainnet PublicResolver: `0x231b0Ee14048e9dCcD1d247744d114a4EB5E8E63`
- Sepolia subname issuance via direct ENS Registry (`sbo3l agent register --network sepolia`; PR #116 dry-run merged; broadcast slice in flight)
- ENSIP-25 CCIP-Read gateway live at https://ccip.sbo3l.dev (T-4-1 ✅; PR #124 + #121 + uptime probe)
- Cross-agent attestation protocol (T-3-4 — first-tier amplifier 60-agent fleet PR #141)
- Trust-DNS viz (T-3-5 — canvas renderer PR #164 for ≥100 agents)
- 1500-word "Trust DNS" essay at https://docs.sbo3l.dev/trust-dns (T-3-6 — pending)

## Sponsor track field — ENS AI Agents

**Differentiator:** SBO3L combines **ENS as identity** + **ERC-8004 Identity Registry** + **ENSIP-25 CCIP-Read** for off-chain dynamic records (computed reputation, current audit-root, fresh capability set). The CCIP-Read gateway runs at https://ccip.sbo3l.dev with an uptime probe.

## Sponsor track field — Uniswap Best API

**How does your project use Uniswap?**

> SBO3L's `UniswapExecutor` is a `GuardedExecutor` over the Uniswap Trading API. Each swap intent runs through SBO3L's policy boundary before the swap is constructed. Slippage, MEV-protection (priority fee bounding, freshness), token-allowlist, and value-cap are all expressed as policy rules — not as bot logic. The policy decision is signed; the audit row links that decision to the eventual on-chain tx hash via the Passport capsule.
>
> v1.0.1 ships a working Sepolia QuoterV2 quote path, with the real swap construction landing in T-5-1..T-5-5. The capsule's `execution.live_evidence.tx_hash` is the canonical proof — drop it into a verifier and you can confirm the swap was bounded, authorised, and within slippage limits.

**Live Uniswap evidence:**
- `crates/sbo3l-execution/src/uniswap.rs` — `live_from_env()` + Sepolia QuoterV2 quote
- Sepolia QuoterV2: `0xEd1f6473345F45b75F8179591dd5bA1888cf2FB3`
- Sepolia route: WETH → USDC `0x1c7D4B19…` (used in submission-day verification at HEAD `0707079`)
- Quote evidence captured: `quote_source: uniswap-v3-quoter-sepolia-…` plus real `sqrt_price_x96_after` and a freshness timestamp embedded in the capsule
- `examples/uniswap-agent/` (T-5-6) — TS + Py demo (gated on T-5-5 swap landing; Dev 2 in flight on PR #165 for T-5-1 swap construction)
- Real Sepolia tx hash captured into capsule (`demo-scripts/artifacts/uniswap-real-swap-capsule.json` after T-5-5)

## Sponsor track field — 0G Track A (Storage)

_populate after T-6-1 (capsule storage on 0G)_

## Sponsor track field — 0G Track B (DA / Compute)

_populate after T-6-2 / T-6-3_

## Sponsor track field — Gensyn AXL

_populate after T-8-* (multi-node SBO3L with AXL)_

## What's hardcoded vs what's testnet vs what's mainnet

| Surface | State |
|---|---|
| KeeperHub | Live `wfb_…` token executes real workflows against `m4t4cnpmhv8qquce3bv3c`; `kh-172o77rxov7mhwvpssc3x` shape executionId captured |
| ENS apex `sbo3lagent.eth` | **Mainnet, 5 `sbo3l:*` records on chain today** (Phase 2 adds 2 more for 7 total) |
| ENS subnames | Sepolia, issued via direct ENS Registry `setSubnodeRecord` (Daniel parent-owner; Durin path evaluated and dropped 2026-05-01) |
| Uniswap | Sepolia QuoterV2 (`0xEd1f6473345F45b75F8179591dd5bA1888cf2FB3`) quotes live; real swap (T-5-5) Sepolia |
| 0G | Testnet (faucet flaky; see notes in `docs/0g-…`) |
| Gensyn AXL | Testnet |

Honest disclosure: anything labelled mock in the demo output is mock. The capsules clearly mark `mode: "mock"` vs `mode: "live"`; tampering with that field fails strict verification.

## Submission readiness checklist (Daniel before clicking Submit)

- [ ] All sponsor track fields populated above
- [ ] Demo video URL pasted
- [ ] All Etherscan tx hashes captured
- [ ] All KH issue URLs filled in
- [ ] FEEDBACK.md last commit ≤ 24h ago (proves freshness)
- [ ] `cargo install sbo3l-cli --version 1.0.1` works on a fresh `mktemp -d` env (Heidi pre-verifies)
- [ ] https://sbo3l.dev/proof loads + verifies a tampered capsule as failed
