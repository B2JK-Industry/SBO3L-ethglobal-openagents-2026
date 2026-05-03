# Voiceover script — SBO3L 3-minute demo

> **Format:** Read this aloud at conversational pace. Total target: **2:55-3:00**.
> **Voice:** Daniel, conversational, no slick. Show the curl, show the response.
> **Pacing:** ~150 words per minute. Total word count below = ~430 words.

> Each section has a target time and a `[CUE]` line telling you what's on screen.
> If you want, record each section separately and stitch in step 4.

---

## Scene 1 — 0:00 → 0:18 (18 seconds, ~45 words)

`[CUE: scene-1-home.mp4 — homepage hero with tagline + ASCII diagram]`

> An autonomous AI agent just decided to spend fifty thousand dollars of someone else's money. Was it allowed to? Who said so? Where's the proof? **Don't give your agent a wallet. Give it a mandate.**

`[Pause 2 seconds for tagline to land on screen]`

---

## Scene 2 — 0:18 → 0:48 (30 seconds, ~75 words)

`[CUE: terminal-scenes.mp4 first half — install + verify-ens + doctor]`

> SBO3L is a Rust workspace published on crates.io. One command installs it. The CLI verifies a real mainnet ENS identity — five sbo3l text records on `sbo3lagent.eth`. Doctor probes seven Sepolia contracts the project deployed: OffchainResolver, AnchorRegistry, SubnameAuction, ReputationBond, ReputationRegistry, ERC-8004 IdentityRegistry, plus the ENS Registry itself. Everything live, everything verifiable from your terminal.

---

## Scene 3 — 0:48 → 1:20 (32 seconds, ~80 words)

`[CUE: scene-3-proof.mp4 — drag-drop capsule into /proof, 6 green checks]`

> Now the load-bearing claim. Every SBO3L decision becomes a Passport capsule — one self-contained JSON file. Drag it into our verifier page in your browser. The same Rust crate, compiled to WebAssembly, runs six cryptographic checks: schema, request hash, policy hash, decision result, signature, audit chain. Zero network calls after page load. **Tamper one byte** — strict-hash verifier rejects it. Your browser is the trust anchor. No SBO3L server required.

---

## Scene 4 — 1:20 → 1:50 (30 seconds, ~75 words)

`[CUE: terminal-scenes.mp4 second half — KH workflow allow + deny]`

> Watch a real KeeperHub workflow execution gated by SBO3L. Agent posts an APRP request. Policy boundary decides allow. Receipt is signed Ed25519. Adapter posts the IP-1 envelope to KeeperHub's webhook. Real `executionId` returned. Now a prompt-injected request — different agent, hostile intent. SBO3L denies before any executor runs. Audit log captures the denial. **KeeperHub never sees it.** Defense at the policy boundary, not after the fact.

---

## Scene 5 — 1:50 → 2:25 (35 seconds, ~85 words)

`[CUE: scene-5-uniswap.mp4 — Etherscan UNI-A1 tx + zoom on swap details]`

> SBO3L isn't just KeeperHub. Here's a real Uniswap V3 mainnet swap from the SBO3L deploy wallet — block twenty-five million, gas seventy-one cents at two-point-one-nine gwei. Five thousandths of an ETH for eleven point five seven USDC via Universal Router. The same swap-policy guard — token allowlist, slippage cap, treasury recipient — protects this swap and any future agent-initiated swap. **Live on Ethereum. Verifiable on Etherscan.** This isn't a quote. It's an executed transaction.

---

## Scene 6 — 2:25 → 3:00 (35 seconds, ~70 words)

`[CUE: scene-6-outro.mp4 — number strip animation + close on tagline]`

> Numbers, last time. Nine hundred seventy-seven tests passing. Ten Rust crates on crates dot i o. Twenty-five npm packages, eight on PyPI, fifteen framework integrations. Five sponsor live integrations: KeeperHub, ENS, Sepolia OffchainResolver, Uniswap mainnet, 0G Storage. ENSIP twenty-six submitted upstream. One self-contained Passport capsule, anyone can verify offline. **SBO3L. Don't give your agent a wallet. Give it a mandate.**

`[Pause 3 seconds — tagline + URL on screen, fade out]`

---

## Recording tips

- **Conversational pace.** Don't rush. ~150 wpm = natural reading speed.
- **Pause at scene breaks.** Easier to splice if each scene's audio is a separate take.
- **Smile when you read the tagline** — your voice carries it.
- **One bad take? Re-record just that scene** — don't restart the whole thing.

## Word count check

- Scene 1: 45 words / 18s
- Scene 2: 75 words / 30s
- Scene 3: 80 words / 32s
- Scene 4: 75 words / 30s
- Scene 5: 85 words / 35s
- Scene 6: 70 words / 35s
- **Total: 430 words / 180s = 143 wpm** (right in the conversational sweet spot)

## Total runtime: 3:00 (hard cap from ETHGlobal — judges DQ videos > 4 min)
