# SBO3L — 3-minute demo video script (v1)

> **Goal:** in 3 minutes a judge believes SBO3L is real, useful, and verifiable.
> **Voice:** Daniel, conversational. No slick. Show the curl, show the response.
> **Audience:** ETHGlobal Open Agents 2026 judges + sponsor reviewers (KH, ENS, Uniswap).
> **Total budget:** 3:00 hard cap. The verifier scene at 1:45 is the load-bearing scene; everything else is setup or close.

---

## 0:00 — 0:15 — Cold open + tagline

> **Voiceover:** "An autonomous agent just decided to spend $50,000 of someone else's money. Was it allowed to? Who said so? Where's the proof?"
> **Screen:** terminal, three `curl` responses scrolling — `decision: allow`, `decision: deny`, `decision: requires_human`. Cut to logo + tagline.
> **Tagline lands on screen + voice:** **Don't give your agent a wallet. Give it a mandate.**

## 0:15 — 0:45 — Live KeeperHub workflow execution + signed receipt

> **Voiceover:** "Watch a real KeeperHub workflow run, gated by SBO3L. The agent submits intent. SBO3L decides. KeeperHub executes the allowed actions only."
> **Screen:**
> 1. APRP request POSTed to local SBO3L → `decision: allow` + signed Ed25519 `policy_receipt.signature`
> 2. SBO3L's `KeeperHubExecutor::live_from_env()` POSTs the IP-1 envelope (`sbo3l_request_hash`, `sbo3l_policy_hash`, `sbo3l_receipt_signature`, `sbo3l_audit_event_id`) to the real workflow webhook `https://app.keeperhub.com/api/workflows/m4t4cnpmhv8qquce3bv3c/webhook`
> 3. KH responds with a real `executionId` of the form `kh-172o77rxov7mhwvpssc3x` (KH-format, not a ULID)
> 4. Cut to denied request: `policy.deny_unknown_provider` — KH never sees the request. The audit chain logs the deny with the same shape as the allow.
> **Voiceover beat:** "Denied actions never reach the sponsor. Allowed actions arrive at KeeperHub with a signed envelope KeeperHub can echo back into their audit row."

## 0:45 — 1:15 — `sbo3lagent.eth` on mainnet + 5-agent Sepolia fleet

> **Voiceover:** "When five agents need to know who they're talking to, SBO3L turns ENS into the agent trust DNS."
> **Screen:**
> 1. `sbo3l agent verify-ens sbo3lagent.eth --rpc-url https://ethereum-rpc.publicnode.com` returns 5 `sbo3l:*` records on mainnet — `agent_id`, `endpoint`, `policy_hash` (`e044f13c5acb…`), `audit_root`, `proof_uri`. Phase 2 adds `capability` and `reputation` for 7 total.
> 2. The `policy_hash` matches the offline fixture byte-for-byte — no drift between published identity and shipped behaviour.
> 3. Cut to Sepolia: 5+ named agents resolved (`research-agent.sbo3lagent.eth`, `trading-agent`, `swap-agent`, `audit-agent`, `coordinator-agent`) — each issued via direct ENS Registry `setSubnodeRecord` (no third-party registrar; we evaluated Durin and dropped it).
> **Voiceover (Kevin caveat preemption — talking head over the verify-ens cut):** "Other ENS-based agent identity schemes leave the records as user-claimed strings — anyone can write anything. SBO3L's `policy_hash` is different: it's a cryptographic commitment to the live engine's actual rules. If they drift, `verify-ens` fails closed. The text record is verifiable, not just claimed."
> **Voiceover beat:** "Same parent, different agents, fully on chain. Anyone with an Ethereum RPC can verify these identities — and SBO3L's CCIP-Read gateway resolves the dynamic ones too."

## 1:15 — 1:45 — Trust DNS viz with live attestation events

> **Voiceover:** "Now watch the agents discover and trust each other in real time."
> **Screen:**
> 1. Open `app.sbo3l.dev/trust-dns` (or the Vercel preview URL fallback while custom domain points)
> 2. Force-directed graph (D3 + canvas renderer for ≥100 agents) — agents appear as nodes, edges form as ENS resolutions happen
> 3. Cross-agent attestation event fires: edge animates with a "signed" badge; the receiving SBO3L verifies sender's pubkey → recipient's published policy → recipient's actual decision
> 4. Tampered attestation injected — node shows red `cross_agent.attestation_invalid` ring; edge does NOT form
> **Voiceover beat:** "Every edge is a signed Ed25519 attestation, time-bound, policy-hash-pinned. A tampered attestation gets rejected at the policy boundary — not after the fact."

## 1:45 — 2:15 — Pasting capsule into `/proof`, 6 ✅ checks

> **Voiceover:** "Now the part that matters. Every action above produced a Passport capsule — a single JSON file. Watch this."
> **Screen:**
> 1. Drag a `capsule.json` file into the `/proof` page on the marketing site
> 2. WASM verifier runs in the page (sbo3l-core compiled to wasm, no daemon, no network) — six green checkmarks pop in: schema, request-hash, policy-hash, decision-result, agent-id, audit-event-id linkage. (Strict-hash verifier additionally checks Ed25519 signatures + audit-chain content hashes.)
> 3. **Tamper one byte** — re-drop the capsule. Same verifier, now red: `capsule.audit_event_hash_mismatch`. The chain rejects the modification.
> **Voiceover beat:** "No daemon. No network. No RPC. Just the agent's published Ed25519 pubkey and the capsule. That's the load-bearing claim of this project."

## 2:15 — 2:45 — Multi-framework agent: LangChain → CrewAI → AutoGen, single audit chain

> **Voiceover:** "Different frameworks. Different runtimes. Same proof."
> **Screen:**
> 1. A single research-agent script runs three steps: a tool call via `@sbo3l/langchain`, a follow-up agent task via `sbo3l-crewai`, a multi-agent vote via `@sbo3l/autogen`. Each call hits the same SBO3L daemon.
> 2. The audit chain on the right shows three events appended in order, each linked by `prev_event_hash` to the previous — no matter which framework the agent was running in when it called.
> 3. The capsule for the final action embeds the full chain prefix back to the first event. One file, three frameworks, cryptographically linked.
> **Voiceover beat:** "Eight framework integrations on day one. LangChain TS and Python, CrewAI, AutoGen, ElizaOS, LlamaIndex, Vercel AI, LangGraph. Whatever your agent stack is, the audit chain is the same."

## 2:45 — 3:00 — Close

> **Voiceover:** "9 crates on crates.io. 8 framework integrations. 60 agents on a Sepolia trust mesh. One mandate. SBO3L. Don't give your agent a wallet. Give it a mandate."
> **Screen:** four numbers fade in (9, 8, 60, 1) then collapse into the tagline + sbo3l.dev URL + GitHub URL.

---

## Storyboard checklist (Daniel pre-record)

- [ ] All sponsor `live_from_env()` paths smoke-tested same morning as record
  - KH `wfb_…` token still valid (workflow `m4t4cnpmhv8qquce3bv3c`; one execution today keeps the cached path warm)
  - Sepolia private key funded — `0xdc7EFA…D231` per memory `alchemy_rpc_endpoints`
  - ENS RPC PublicNode mainnet + Sepolia per memory `live_rpc_endpoints_known`
- [ ] Capsule for tamper demo pre-prepared (`/tmp/capsule-tamper-demo.json`) — generate via `sbo3l passport run … --out` then byte-flip one char in `audit_event_hash`
- [ ] Sepolia agent fleet (T-3-3 #138) merged + run, so 5 agents show up in the trust-dns viz
- [ ] Multi-framework script ready: pre-recorded fixture run captured into a JSON transcript so the on-camera replay is deterministic (timing budget for 30s is tight — pre-record + voice-over)
- [ ] Recording at 1080p minimum, screen-share crisp, terminal font ≥ 18pt
- [ ] Custom domain (`sbo3l.dev`) pointed before record OR fallback Vercel preview URL pre-loaded in the browser tab order

## Cuts to consider if over 3:00

1. Drop the cold-open three-`curl`-responses scroll (15s saved); land tagline first
2. Combine ENS + trust-DNS-viz into a single 30s segment instead of 30s + 30s
3. Cut the multi-framework crossover (the most ambitious beat — keep only if pre-recorded transcript runs cleanly)

## Cuts NOT to make

The verifier scene at 1:45–2:15. That's the load-bearing scene; the rest is setup. The tampered-byte demonstration is non-negotiable.

## Outro fact-check (every number is falsifiable)

- **9 crates** — `cargo search sbo3l-` returns 9 results at 1.2.0
- **8 framework integrations** — npm `@sbo3l/{sdk,langchain,autogen,elizaos,vercel-ai,design-tokens}` + PyPI `sbo3l-{sdk,langchain,crewai,llamaindex,langgraph}`. Counted as integrations: LangChain TS, LangChain Py, CrewAI, AutoGen, ElizaOS, LlamaIndex, Vercel AI, LangGraph = 8.
- **60 agents** — refers to T-3-4 first-tier amplifier (PR #141 — 60-agent fleet config)
- **1 mandate** — wordplay on the tagline (lowercase noun, brand uppercase = SBO3L)

---

## Visual assets (Wave 2 design kit)

All recording-ready assets live in `apps/marketing/public/demo-assets/`. They render standalone (no JS, no React, no build step — just SVG), so Daniel can drop them straight into the recording timeline (Final Cut, Premiere, DaVinci, OBS — anything that imports SVG or rasterises on import).

| File | When to use | Notes |
|---|---|---|
| `title-card.svg` | 0:00 — opening hold (~3s) | 1920×1080. Animated particle drift via inline `<style>`. Honors `prefers-reduced-motion`. |
| `lower-third-template.svg` | 0:00 — name-tag while VO plays | 600×100. Hand-edit `Daniel Babjak` / `Submission Lead` strings to repurpose for sponsor reps if needed. |
| `sponsor-insert-keeperhub.svg` | 0:15 — KH workflow scene | 200×200. Shield motif. |
| `sponsor-insert-ens.svg` | 0:45 — ENS subname scene | 200×200. Namespace tree motif. |
| `sponsor-insert-uniswap.svg` | (cut from current 3-min cut, hold for v2) | 200×200. Swap arrows motif. |
| `sponsor-insert-anthropic.svg` | (hold for sponsor-track-specific cut) | 200×200. Tool-use chat motif. |
| `end-card.svg` | 2:45 — outro hold (~5s) | 1920×1080. Stylized QR placeholders by default. |
| `qr-github.svg` / `qr-npm.svg` / `qr-cratesio.svg` | end-card composite | Real machine-readable QRs. Generated by `npm --prefix apps/marketing run build:qr`. Composite over `end-card.svg` placeholders in editor, or splice as separate clips. |

### Regenerating QR codes

If the GitHub repo URL, npm org, or crates.io crate name changes:

1. Edit `apps/marketing/scripts/build-qr.mjs` (the `targets` array)
2. `cd apps/marketing && npm run build:qr`
3. Re-export the end-card composite from the editor

The script depends on `qrcode` (devDep, already in `package.json`). Error-correction level M, margin 1, black-on-white — high enough density that a phone camera locks within 2-3 seconds at 1080p source.

### Brand fidelity

All assets use the design-tokens palette:
- background `#0a0a0f` (token: `surface.canvas`)
- accent `#4ad6a7` (token: `accent.mint`)
- foreground `#e6e6ec` (token: `text.primary`)
- muted `#9999a8` (token: `text.muted`)
- border `#2a2a3a` (token: `border.subtle`)

Font is `ui-monospace, monospace` — system mono on every recording machine. No webfont fetch, no FOUT.

### Render verification

Manual cross-browser check completed; full notes at `docs/dev1/video-asset-render-notes.md`. Summary: all assets render cleanly in Chrome + Safari at 1× and 2× DPI, with the title-card animation honoring `prefers-reduced-motion`.
