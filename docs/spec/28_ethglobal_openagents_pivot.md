# Mandate ETHGlobal Open Agents Pivot Analysis

**Datum:** 2026-04-26  
**Ucel:** Vyhodnotit, ci **Mandate** (technical namespace `mandate`) dava vacsi zmysel submitnut do ETHGlobal Open Agents namiesto alebo popri ETHPrague.  
**Important:** Verejna stranka eventu a prizes page su aktualny ciel strategickeho rozhodovania. Pred finalnym build planom treba este raz overit presny rok/datum v ETHGlobal dashboarde alebo Discorde, lebo verejny scrape moze ukazovat cache/nekonzistentny datum.

---

## 1. Short verdict

Ak je Open Agents aktualny/upcoming event, **Mandate** ma velmi dobry fit a moze byt dokonca prirodzenejsi target nez ETHPrague pre agent-infra bounty.

ETHPrague je lepsi pre:

- hlavnu cenu typu Network Economy,
- fyzicke stage demo,
- broader crypto/economy narrative,
- Umia/FinTechers-style agentic venture + fintech angle.

Open Agents je lepsi pre:

- agent framework/tooling,
- MCP/A2A/agent-to-agent infra,
- ENS identity for agents,
- execution/reliability layer for agents,
- Uniswap/DeFAI agent actions,
- decentralizovanu agent komunikaciu.

Najlepsi tah nie je projekt zahodit ani prepisat. Najlepsi tah je mat **Open Agents mode**:

> Mandate as the policy, identity and execution firewall for open AI agents.

---

## 2. Open Agents prize fit

### KeeperHub - strongest fit

KeeperHub sa popisuje ako execution and reliability layer for AI agents operating onchain, s retry logic, gas optimization, private routing and full audit trails. To je velmi blizko nasmu problemu.

**Winning frame:**

> Mandate decides, KeeperHub executes.

Agent neposiela transakcie priamo. Agent posle payment/action intent do mandate. Vault overi policy, budget, identity a audit, potom povoli KeeperHub execution.

**Scope add:**

- `/crates/mandate-execution/src/keeperhub.rs`
- `/demo-scripts/sponsors/keeperhub-guarded-execution.sh`
- MCP/CLI adapter: approved requests can be routed into KeeperHub.

**Chance if working:** high. Toto je najprirodzenejsi Open Agents sponsor target.

### ENS - very strong fit

Open Agents ENS bounty chce, aby ENS robilo realnu pracu pre agent identity/discoverability, nie dekoraciu.

**Winning frame:**

> ENS name resolves to agent identity, vault endpoint, policy hash and audit root.

**Scope add:**

- functional resolver check, nie hard-coded output,
- text records:
  - `mandate:agent_id`
  - `mandate:policy_hash`
  - `mandate:audit_root`
  - `mandate:endpoint`

**Chance if working:** high.

### Uniswap Foundation - medium/strong fit

Uniswap chce agentic finance cez Uniswap API: agents swap/settle value onchain with transparency and execution.

`mandate` by nemal byt trading agent. Ale moze byt **guardrail for trading agents**.

**Winning frame:**

> Agent can request a Uniswap swap, but mandate enforces token allowlists, slippage, per-tx caps, treasury policy and policy receipts before execution.

**Scope add:**

- `/demo-scripts/sponsors/uniswap-guarded-swap.sh`
- policy checks:
  - allowed input/output token,
  - max slippage,
  - max notional,
  - quote freshness,
  - recipient/treasury guard.
- `FEEDBACK.md` in repo root if targeting Uniswap bounty.

**Chance if working:** medium/high, but only if Uniswap API integration is real.

### Gensyn AXL - medium/strong fit

Gensyn AXL wants peer-to-peer encrypted agent communication with MCP/A2A support, no central coordinator.

**Winning frame:**

> A buyer agent and a seller agent communicate over AXL. mandate controls whether the buyer agent may pay the seller agent.

**Scope add:**

- two separate AXL nodes,
- buyer research agent,
- seller paid-data/API agent,
- payment intent routed through mandate,
- policy receipt returned over AXL.

**Chance if working:** medium/high. Integration depth matters.

### 0G - medium fit

0G tracks are split:

- framework/tooling/core extensions,
- autonomous agents/swarms/iNFT innovations.

`mandate` fits better as framework/tooling than as a pure autonomous agent.

**Winning frame:**

> mandate is a payment and policy extension for open agent frameworks deployed with 0G storage/compute.

**Scope add options:**

- policy receipts stored in 0G Storage,
- agent venture passport stored in 0G Storage,
- optional sealed inference / 0G Compute as evidence source,
- example OpenClaw/LangChain agent using mandate plugin.

**Chance if working:** medium. Requires real 0G usage, not just branding.

---

## 3. Revised product frame for Open Agents

For Open Agents, avoid "agent venture" first. Use:

> **Open Agent Payment Firewall**

or:

> **Policy firewall for agents that can spend.**

Pitch:

> Open agents are about to communicate, trade, swap and execute onchain. mandate gives every agent a local policy firewall, signed receipts and a safe execution boundary.

This is narrower and more technical than ETHPrague's Network Economy/Agent Venture framing.

---

## 4. Scope changes for Open Agents mode

### Required

1. Agent framework adapter:
   - LangChain/LangGraph or OpenClaw-style tool wrapper.
   - Agent calls `mandate.request_payment()` or `mandate.request_action()`.

2. MCP/A2A-ready interface:
   - expose vault actions as MCP tools,
   - optional AXL transport.

3. Policy receipts:
   - signed proof of allow/deny,
   - returned to agent and optionally to counterparties.

4. ENS functional identity:
   - resolve agent name,
   - fetch policy/audit metadata.

### Sponsor-specific add-ons

| Sponsor | Add-on | Risk |
|---|---|---|
| KeeperHub | approved execution backend | medium |
| Uniswap | guarded swap action | medium |
| Gensyn | AXL buyer/seller agents | medium/high |
| 0G | store passport/receipts or framework plugin | medium/high |
| ENS | agent identity proof | low/medium |

---

## 5. Architecture impact

The core architecture stays intact:

- agent request boundary,
- policy engine,
- budget ledger,
- signer isolation,
- audit hash chain.

Open Agents mode adds adapter and execution layers:

```text
Agent framework / MCP / A2A / AXL
          |
          v
Mandate Agent Gateway (`mandate` technical service)
          |
          v
Policy + Budget + Risk Engine
          |
          +--> Policy Receipt Service
          |
          +--> ENS / Agent Identity Resolver
          |
          +--> Execution Router
                    |
                    +--> KeeperHub
                    +--> Uniswap API
                    +--> x402 provider
                    +--> direct signer fallback
```

New modules:

- `/crates/mandate-mcp/`
- `/crates/mandate-execution/`
- `/crates/mandate-identity/ens.rs`
- `/crates/mandate-receipts/`
- `/demo-scripts/sponsors/{keeperhub,uniswap,gensyn-axl,ens,0g}-*.sh`

---

## 6. Probability estimate if a similar Open Agents event is active

Assuming working demo + sponsor integrations:

| Prize | Estimated chance |
|---|---:|
| KeeperHub Best Use | **45-60%** |
| ENS Best Integration for AI Agents | **40-55%** |
| Uniswap API integration | **25-40%** |
| Gensyn AXL | **25-40%** |
| 0G Framework/Tooling | **20-35%** |
| 0G Autonomous Agents | **15-30%** |

Portfolio estimate:

- at least one prize: **75-90%**
- two prizes: **40-60%**
- three or more: **15-30%**

These numbers assume the integration is real and visible. Without real sponsor integrations, chances drop sharply.

---

## 7. ETHPrague vs Open Agents decision

| Criterion | ETHPrague | Open Agents |
|---|---:|---:|
| Agent/security product fit | strong | very strong |
| Main prize narrative | strong via Network Economy | depends on event structure |
| Sponsor fit | Umia/FinTechers/ENS | KeeperHub/ENS/Uniswap/Gensyn/0G |
| Demo complexity | medium | medium/high because sponsor integrations matter |
| Chance to win one prize | high if polished | very high if active and integrated |
| Chance to win several prizes | medium/high | high, but integrations fragment focus |
| Product direction alignment | agent venture security | open agent execution/security tooling |

**Verdict:**  
If a new Open Agents event is open, submit there too or prioritize it if timeline fits. The project is arguably an even more native fit for Open Agents, but only after adding adapter/execution integrations.

---

## 8. Recommended dual-track strategy

Build one core:

> Mandate: policy-bound payment and execution firewall for AI agents.

Then package it two ways:

### ETHPrague package

- Agent Venture Firewall
- Network Economy
- Umia/FinTechers/ENS
- stage demo with real agent + attack + audit

### Open Agents package

- Open Agent Payment Firewall
- MCP/A2A/AXL-ready
- KeeperHub/Uniswap/ENS integrations
- developer-facing framework adapter

This is the same architecture with different demo cuts.

---

## 9. Sponsor-specific narrow scope plan

### Principle

Pre Open Agents nestavat "universal agent security platform" v pitchi. Postavit jeden core a pre kazdeho sponzora ukazat uzky, realny adapter:

> one Mandate core (`mandate` technical namespace), five sponsor-native entrypoints.

Core:

- APRP/payment intent.
- Policy engine.
- Budget/risk checks.
- Decision token.
- Policy receipt.
- Audit hash chain.
- ENS agent identity metadata.

Sponsor adapters:

- KeeperHub execution router.
- Uniswap guarded swap.
- Gensyn AXL buyer/seller payment.
- 0G agent framework/storage proof.
- ENS identity resolver.

### KeeperHub scope

**Goal:** vyhrat Best Use of KeeperHub.

**Narrow product frame:**

> Mandate is the pre-execution risk and policy layer for KeeperHub-powered agents.

**Build only this:**

1. Agent requests an onchain action/payment.
2. mandate validates:
   - agent identity,
   - budget,
   - recipient,
   - chain/action allowlist,
   - policy hash.
3. Mandate emits signed policy receipt.
4. Approved action is routed to KeeperHub CLI/API/MCP.
5. Denied action never reaches KeeperHub.

**Demo line:**

> KeeperHub guarantees execution. Mandate guarantees the agent was allowed to ask for it.

**Do not build:** our own retry/gas/private-routing layer. That competes with KeeperHub instead of complementing it.

### ENS scope

**Goal:** vyhrat Best ENS Integration for AI Agents.

**Narrow product frame:**

> ENS is the agent's public identity; Mandate is the enforcement behind that identity.

**Build only this:**

1. Agent has ENS name: `research-agent.team.eth`.
2. ENS records resolve:
   - `mandate:agent_id`,
   - `mandate:endpoint`,
   - `mandate:policy_hash`,
   - `mandate:audit_root`,
   - `mandate:receipt_schema`.
3. Demo fetches records live or through a local resolver pointed at testnet records.
4. The trust badge proves the policy hash on ENS matches the active mandate policy.

**Demo line:**

> You can discover an agent, but also discover what it is allowed to spend.

**Do not build:** cosmetic ENS display only. ENS must gate/discover/verify something.

### Uniswap scope

**Goal:** compete for Best Uniswap API integration.

**Narrow product frame:**

> Mandate is a guarded swap firewall for trading agents.

**Build only this:**

1. Agent asks: "Swap 5 USDC to ETH."
2. Mandate calls/uses Uniswap API quote.
3. Mandate enforces:
   - max trade size,
   - token allowlist,
   - max slippage,
   - quote freshness,
   - treasury recipient,
   - daily budget.
4. Approved swap gets policy receipt.
5. Attack prompt tries to swap into a denied token or excessive slippage; vault denies.
6. Add required `FEEDBACK.md` for Uniswap API builder feedback.

**Demo line:**

> Agentic finance is only useful if agents can trade within enforceable limits.

**Do not build:** trading strategy. We are not an alpha bot; we are the risk layer for agentic swaps.

### Gensyn AXL scope

**Goal:** compete for Best Application of Agent eXchange Layer.

**Narrow product frame:**

> AXL lets agents talk peer-to-peer; Mandate lets them pay peer-to-peer safely.

**Build only this:**

1. Buyer agent runs on AXL node A.
2. Seller/data agent runs on AXL node B.
3. Buyer requests a paid report/API result.
4. Payment intent moves through AXL.
5. Mandate checks and signs/denies.
6. Policy receipt returns over AXL.

**Demo line:**

> No central broker, no hot wallet agent, no blind trust.

**Do not build:** full agent social network or simulation. Only buyer/seller paid interaction.

### 0G scope

**Goal:** compete for 0G framework/tooling or autonomous agents track.

**Narrow product frame:**

> Mandate is a payment/policy module that open agent frameworks can plug into.

**Build only this:**

Option A - Tooling track:

- OpenClaw/LangChain-style plugin: `mandate.request_payment`.
- Agent memory/receipts stored in 0G Storage.
- Example agent deployed with one working paid action.

Option B - Autonomous agents track:

- Research agent stores persistent venture passport or receipts in 0G Storage.
- Optional 0G Compute inference output hash linked to receipt.

**Demo line:**

> 0G hosts the agent's memory/compute layer; Mandate controls its economic actions.

**Do not build:** a new agent framework from scratch. A plugin/core extension has better scope discipline.

---

## 10. Recommended Open Agents prize priority

| Priority | Sponsor | Why |
|---|---|---|
| 1 | **KeeperHub** | Best conceptual fit: execution layer needs pre-execution policy/audit. |
| 2 | **ENS** | Low implementation cost, high narrative value for agent identity. |
| 3 | **Uniswap** | Strong if we build guarded swap; medium risk. |
| 4 | **Gensyn AXL** | Strong story, but integration complexity is higher. |
| 5 | **0G** | Potentially big prize pool, but scope can drift fast. Use plugin/storage proof only. |

Best multi-prize shape:

1. KeeperHub guarded execution.
2. ENS identity proof.
3. Uniswap guarded swap or Gensyn AXL buyer/seller payment.

Do not try to deeply optimize for all five. Build all five as visible thin adapters only if core is already stable.
