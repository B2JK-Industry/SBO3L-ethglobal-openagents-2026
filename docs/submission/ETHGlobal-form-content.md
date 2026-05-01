# ETHGlobal Open Agents 2026 — form content (paste-ready)

> **Audience:** Daniel, at submission time.
> **Outcome:** every track form filled in under 10 minutes by copy-pasting from this file.
> **Voice rule:** every claim in here has a code reference, a live URL, or a runnable command. Honest-over-slick.

The form fields below are derived from typical ETHGlobal submission forms. If the actual track form has different field names, map by intent.

## Project basics (shared across tracks)

**Project name:** SBO3L

**Tagline:** Don't give your agent a wallet. Give it a mandate.

**One-line pitch:** SBO3L is the cryptographically verifiable trust layer for autonomous AI agents — every agent action passes through SBO3L's policy boundary and produces a self-contained Passport capsule anyone can verify offline.

**Project description (~250 words):**

> SBO3L is a trust layer that sits between an autonomous AI agent and the systems it acts on. Instead of giving the agent a wallet or signing keys, you give it a *mandate*: a policy decision, a signed receipt, and an audit-chain entry — produced by SBO3L the moment the agent submits its intent.
>
> The core wire format is APRP (Agent Payment Request Protocol): a payment-shaped JSON envelope with `intent`, `amount`, `chain`, `expiry`, `risk_class`, `nonce`. SBO3L canonicalises it (JCS), hashes it (SHA-256), runs a deterministic policy decision against an Ed25519-signed active policy, increments multi-scope budgets, claims the nonce, signs a `PolicyReceipt`, appends a hash-chained `AuditEvent`, and routes through a sponsor adapter (KeeperHub webhook, Uniswap quoter, etc.).
>
> The output is a *Passport capsule v2* — a single JSON file containing every byte needed to re-derive the decision offline. With one CLI command (`sbo3l passport verify --strict`) anyone can independently confirm: schema, request-hash, policy-hash, decision-result, agent-id, audit-event-id, Ed25519 signatures, audit-chain linkage. No daemon, no network, no RPC. Same WASM verifier runs in the browser at sbo3l.dev/proof.
>
> v1.0.1 ships nine Rust crates on crates.io, SDKs on npm and PyPI, six framework integrations (LangChain TS/Py, AutoGen, CrewAI, ElizaOS, LlamaIndex), bonus adapters (Vercel AI, LangGraph), Docker, marketing site, hosted preview, docs site, and a CCIP-Read gateway. KeeperHub, ENS mainnet, and Uniswap Sepolia all have working `live_from_env()` smokes.

**Repo URL:** https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026

**Live demo URL:** https://sbo3l.dev

**Verifier (judges click this):** https://sbo3l.dev/proof

**Demo video URL:** _populate after recording_

**Built during the hackathon? (yes/no):** Yes — entire codebase shipped within the 100-day window. Repo init at `9504fa7`; v1.0.1 release commit at `c90f571`.

## Sponsor track field — KeeperHub Best Use

**How does your project use KeeperHub?**

> Every action that flows through SBO3L's `KeeperHubExecutor` is gated by an APRP policy decision before any KeeperHub workflow webhook is called. Allowed actions arrive at KH with a signed `sbo3l_*` envelope (request_hash, policy_hash, receipt_signature, audit_event_id, optional passport_uri) — five optional fields KH can echo back to make every execution row cryptographically linked to the upstream authorisation.
>
> SBO3L ships a standalone `sbo3l-keeperhub-adapter` crate on crates.io implementing IP-1..IP-5 from the integration paths doc. The adapter has both `local_mock()` (CI-safe default) and `live_from_env()` (real `wfb_…` token + workflow id) constructors. Live submissions to KeeperHub were exercised end-to-end as part of the demo gate — execution id `kh-<ULID>` is captured into the Passport capsule for offline verification.

**Live integration evidence:** [`demo-scripts/sponsors/keeperhub-real-execution.sh`](../../demo-scripts/sponsors/keeperhub-real-execution.sh) + [`crates/sbo3l-keeperhub-adapter/`](../../crates/sbo3l-keeperhub-adapter/)

**5+ specific KH improvement issues filed:** _populate from `FEEDBACK.md` after T-2-1 lands_

## Sponsor track field — KeeperHub Builder Feedback

**Concrete pain points hit during live integration:** see [`FEEDBACK.md`](../../FEEDBACK.md). Specific suggestions filed as issues on the KeeperHub repo (links in the same file).

## Sponsor track field — ENS Most Creative

**How does your project use ENS?**

> SBO3L turns ENS into *the agent trust DNS*. Every named agent gets a subname under `sbo3lagent.eth` (mainnet apex) with seven `sbo3l:*` text records: `agent_id`, `endpoint`, `policy_hash`, `audit_root`, `proof_uri`, `capability` (new in Phase 2), `reputation` (new in Phase 2, computed from the audit chain).
>
> An agent resolves another agent by ENS name and gets back a complete trust profile — *without trusting any single party*. Cross-agent attestations are signed Ed25519 envelopes that pin the recipient's expected `policy_hash` and an `expires_at`; the receiving SBO3L instance verifies the chain (sender's pubkey → recipient's published policy → recipient's actual decision) before allowing the delegated action.
>
> Five named agents on Sepolia (`research-agent`, `trading-agent`, `swap-agent`, `audit-agent`, `coordinator-agent`) discover each other in real time in the [trust-DNS visualization](https://app.sbo3l.dev/trust-dns). The visualization is the demo video centerpiece.

**Live ENS evidence:**
- Mainnet `sbo3lagent.eth` with 7 sbo3l:* records (resolve via any ENS gateway)
- Sepolia subname issuance via Durin (PR #116 dry-run, full live in T-3-1 main)
- Cross-agent attestation protocol (T-3-4)
- Trust-DNS viz (T-3-5)
- 1500-word "Trust DNS" essay at https://docs.sbo3l.dev/trust-dns (T-3-6)

## Sponsor track field — ENS AI Agents

**Differentiator:** SBO3L combines **ENS as identity** + **ERC-8004 Identity Registry** + **ENSIP-25 CCIP-Read** for off-chain dynamic records (computed reputation, current audit-root, fresh capability set). The CCIP-Read gateway runs at https://ccip.sbo3l.dev with an uptime probe.

## Sponsor track field — Uniswap Best API

**How does your project use Uniswap?**

> SBO3L's `UniswapExecutor` is a `GuardedExecutor` over the Uniswap Trading API. Each swap intent runs through SBO3L's policy boundary before the swap is constructed. Slippage, MEV-protection (priority fee bounding, freshness), token-allowlist, and value-cap are all expressed as policy rules — not as bot logic. The policy decision is signed; the audit row links that decision to the eventual on-chain tx hash via the Passport capsule.
>
> v1.0.1 ships a working Sepolia QuoterV2 quote path, with the real swap construction landing in T-5-1..T-5-5. The capsule's `execution.live_evidence.tx_hash` is the canonical proof — drop it into a verifier and you can confirm the swap was bounded, authorised, and within slippage limits.

**Live Uniswap evidence:**
- `crates/sbo3l-execution/src/uniswap.rs` — `live_from_env()` + Sepolia QuoterV2 quote
- `examples/uniswap-agent/` (T-5-6) — TS + Py demo
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
| KeeperHub | Live `wfb_…` token executes real workflows; `kh-<ULID>` captured |
| ENS apex `sbo3lagent.eth` | Mainnet, 7 sbo3l:* records on chain |
| ENS subnames | Sepolia, issued via Durin |
| Uniswap | Sepolia QuoterV2 quotes live; real swap (T-5-5) Sepolia |
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
