# SBO3L — 3-minute demo video script (DRAFT)

> **Goal:** in 3 minutes, a judge believes SBO3L is real, useful, and verifiable.
> **Voice:** Daniel, conversational. No slick. Show the curl, show the response.
> **Audience:** ETHGlobal judges + sponsor reviewers who haven't seen the README.

## Cold open (0:00 — 0:15)

> **Voiceover:** "An autonomous agent just decided to spend $50,000 of someone else's money. Was it allowed to? Who said so? Where's the proof?"
> **Screen:** Terminal showing an agent calling `curl POST /v1/payment-requests` — three responses scroll: `allow`, `deny`, `requires_human`.

## The hook — "Don't give your agent a wallet" (0:15 — 0:30)

> **Voiceover:** "SBO3L is the cryptographically verifiable trust layer for autonomous AI agents. Every action your agent takes passes through SBO3L's policy boundary first. The output is a Passport capsule. Anyone can verify it. Offline. From a single JSON file."
> **Screen:** Architecture diagram zoom — agent → SBO3L policy → KeeperHub/Uniswap/ENS adapters. Tagline lands: **Don't give your agent a wallet. Give it a mandate.**

## Live integration #1 — KeeperHub (0:30 — 1:00)

> **Voiceover:** "Watch a real KeeperHub workflow run, gated by SBO3L."
> **Screen:**
> 1. Show APRP request hitting SBO3L → `decision: allow`
> 2. Show the `sbo3l_*` envelope POSTed to KeeperHub workflow webhook (real `wfb_…` token, real execution id `kh-<ULID>`)
> 3. Show the executionId echoed back, plus the audit chain entry that links them
> 4. Show denied request: `policy.deny_unknown_provider` — KH never sees the request
> **Voiceover beat:** "Denied actions never reach the sponsor. Allowed actions arrive with a signed envelope KH can echo back into their audit row."

## Live integration #2 — Uniswap (1:00 — 1:30)

> **Voiceover:** "Same shape on a real Sepolia swap."
> **Screen:**
> 1. APRP → SBO3L → `quoteExactInputSingle` against Sepolia QuoterV2
> 2. Real Sepolia tx hash on Etherscan
> 3. Capsule contains the `tx_hash` and the quote evidence — re-verify offline shows the swap was authorised, recorded, and matches the quoted price within slippage bounds
> **Voiceover beat:** "The capsule contains the tx hash. Tomorrow, an auditor can prove this swap was bounded, authorised, and within MEV-safe slippage — without trusting our daemon being online."

## Live integration #3 — ENS Trust DNS (1:30 — 2:00)

> **Voiceover:** "When five agents need to know who they're talking to, SBO3L turns ENS into the agent trust DNS."
> **Screen:**
> 1. Mainnet `sbo3lagent.eth` resolving 7 `sbo3l:*` text records
> 2. Sepolia agent fleet — 5 named agents resolved via Durin
> 3. Trust-DNS visualization: D3 force-directed graph, agents discovering each other in real time, attestation edges signing on the wire
> **Voiceover beat:** "Cross-agent attestations are signed, time-bound, and policy-hash-pinned. A tampered attestation gets `cross_agent.attestation_invalid`."

## The verifier (2:00 — 2:30)

> **Voiceover:** "Now the part that matters. Every action above produced a Passport capsule. Watch this."
> **Screen:**
> 1. Drag a capsule.json file into https://sbo3l.dev/proof
> 2. WASM verifier runs in the page — green checkmarks for: schema, request-hash, policy-hash, decision-result, agent-id, audit-event-id, signature, hash chain
> 3. **Tamper one byte** — same drop, now red: `capsule.audit_event_hash_mismatch`
> **Voiceover beat:** "No daemon. No network. No RPC. Just the agent's published Ed25519 pubkey and the capsule. That's the load-bearing claim of this project."

## What you can install today (2:30 — 2:50)

> **Voiceover:** "If you want to try this — three commands."
> **Screen:**
> ```bash
> cargo install sbo3l-cli --version 1.0.1
> npm install @sbo3l/sdk
> pip install sbo3l-sdk
> ```
> **Voiceover:** "Nine Rust crates on crates.io. SDKs on npm and PyPI. Six framework integrations: LangChain, AutoGen, CrewAI, ElizaOS, LlamaIndex, Vercel AI. Bonus LangGraph adapter. Docker compose, marketing site, hosted preview, docs site, public verifier all live and linked."

## Close (2:50 — 3:00)

> **Voiceover:** "SBO3L. The trust layer for agents. Don't give your agent a wallet. Give it a mandate."
> **Screen:** sbo3l.dev URL + GitHub URL + tagline.

---

## Storyboard checklist (Daniel pre-record)

- [ ] All four sponsor `live_from_env()` paths smoke-tested same morning as record (KH wfb token, Sepolia private key, ENS RPC, mainnet)
- [ ] Capsule for tamper demo pre-prepared (`/tmp/capsule-tamper-demo.json`)
- [ ] Etherscan window pre-loaded with the real Sepolia swap tx
- [ ] Trust-DNS viz pre-warmed with 5+ agents already resolved
- [ ] Recording at 1080p minimum, screen-share crisp, terminal font ≥ 18pt
- [ ] Re-record any segment longer than its allotted slice; total cap = 3:00

## Cuts to consider if over 3:00

1. Drop the architecture diagram in the hook (15s saved)
2. Combine ENS + cross-agent into a single segment (15s)
3. Cut the install commands — link them in the description instead (10s)

## Cuts NOT to make

The verifier scene at 2:00–2:30. That's the load-bearing scene; the rest is setup.
