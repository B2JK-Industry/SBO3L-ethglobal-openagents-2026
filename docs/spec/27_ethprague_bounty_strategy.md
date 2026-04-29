# SBO3L ETHPrague 2026 Bounty Strategy

**Datum:** 2026-04-26  
**Ucel:** Jednoznacne urcit, na ktore ETHPrague 2026 ceny **SBO3L** (technical namespace `mandate`) cieli, na ktore necieli a ake minimum scope treba mat, aby submission nebol iba technicky dobry, ale aj strategicky trafeny.  
**Source check:** ETHPrague Devfolio prizes + sponsor bounty pages, overit znovu 7 dni pred submission.

---

## 1. Hlavny smer

**SBO3L** nesutazi ako "wallet app". Sutazi ako **bezpecnostna a ekonomicka infrastruktura pre autonomous agent economy**.

Hlavny claim:

> AI agents can become economic actors, but they should not hold wallets. They need a local payment firewall, policy engine, signer boundary and audit layer.

Slovensky:

> Agent moze zarabat a minat, ale private key nikdy nema byt v agentovi.

---

## 2. Prize portfolio

### Tier A - hlavne cielene ceny

| Prize | Ideme? | Preco |
|---|---:|---|
| **ETHPrague Network Economy** | YES - primary main prize | Najsilnejsi main-track fit: agenti ako ekonomicki akteri, x402 platby, policy-bound spend, audit, local sovereignty. |
| **Umia - Best Agentic Venture** | YES - primary sponsor prize | SBO3L je treasury/payment safety layer pre agentic ventures. Treba ukazat agent venture loop, nie iba isolated security demo. |
| **The FinTechers Track** | YES - primary sponsor prize | Payments, money movement, risk/compliance, audit trail, policy engine, AI guardrails. Velmi silny fit. |

### Tier B - sekundarne, vysoko efektivne

| Prize | Ideme? | Preco |
|---|---:|---|
| **ENS - Best ENS Integration for AI Agents** | YES - secondary | Lacne rozsireni: `research-agent.team.eth` + text records pre policy hash, vault endpoint, agent id. |
| **Best Privacy by Design** | YES - secondary | Core produkt uz splna: agent nevidi key, seed, treasury ani raw signing credential. |
| **Best Hardware Usage** | YES - secondary | Ak ukazeme Nitrokey/SoftHSM/TPM alebo fyzicky kill switch. |

### Tier C - conditional

| Prize | Ideme? | Podmienka |
|---|---:|---|
| **SpaceComputer** | MAYBE | Iba ak ich KMS/cTRNG/hardware API vieme pouzit bez ohrozenia core demo. Fit: secure signing, tamper-proof logging. |
| **Sourcify** | MAYBE | Iba ako maly contract safety pre-check: vault odmietne calldata na neovereny alebo neznamy contract. Nie je core. |
| **Future Society** | MAYBE | Submitnut sekundarne, ale demo neoptimalizovat na tento track. Narrative: safer autonomous agents for society. |

### Tier D - cielene nie

| Prize | Ideme? | Preco nie |
|---|---:|---|
| **Ethereum Core** | NO | Nie sme client, protocol, EVM, consensus ani core developer tooling. |
| **Swarm bounties** | NO | Slaby fit a male bounty. Audit export na Swarm by posobil ako dodatocny hack, nie ako core produkt. |
| **Most Creative ENS** | NO primary | ENS pouzijeme, ale ciel je AI agent identity bounty, nie kreativny ENS art/use-case. |

---

## 3. Scope-fit verdict

| Target | Dnes splnene? | Gap |
|---|---:|---|
| Network Economy | YES | Treba iba jasnejsie stage narrative: agent economy needs spend governance. |
| Umia Agentic Venture | PARTIAL | Chyba explicitny "agent venture loop": agent ma venture budget/treasury, nakupi paid resource a vytvori output. |
| The FinTechers | YES | Doplnit risk/compliance dashboard moment: budget, deny reason, audit export, policy hash. |
| ENS AI Agents | PARTIAL | Chyba konkretna ENS demo acceptance: text records + resolver check. |
| Privacy by Design | YES | Ukazat na obrazovke, co agent nevidi: no seed, no key, no signer access. |
| Hardware Usage | PARTIAL | Kill switch je scoped; HSM/TPM je R2. Pre hackathon staci SoftHSM/Nitrokey fallback, ak fyzicky hardware nestihneme. |
| SpaceComputer | NO/PARTIAL | Bez ich API/KMS integracie je to iba story, nie bounty-grade proof. |
| Sourcify | NO/PARTIAL | Potrebujeme maly verified-contract pre-check, inak nie je dostatocny use of Sourcify. |

---

## 4. Scope decision

**Aktualna dokumentacia a scope staci na Network Economy, The FinTechers, Privacy by Design a zakladny Hardware Usage.**

**Na Umia Best Agentic Venture by som scope mierne rozsiril.** Nie velkym marketplace buildom, ale tenkym "agent venture overlay":

1. `demo-agents/research-agent` dostane venture profile:
   - agent name,
   - venture goal,
   - daily treasury budget,
   - allowed providers,
   - allowed recipients,
   - ENS identity.
2. Demo ukaze:
   - agent dostane ulohu,
   - agent chce kupit paid API/data/compute,
   - vault schvali legit spend,
   - agent vytvori output,
   - prompt-injection spend je odmietnuty,
   - audit dokazuje, co sa stalo.
3. Dashboard/CLI ukaze:
   - `venture: research-agent.team.eth`,
   - `budget_remaining`,
   - `policy_hash`,
   - `spend_allowed`,
   - `spend_denied`,
   - `audit_chain_ok`.

Toto je dost na "agentic venture" bez toho, aby sme stavali full Umia marketplace.

---

## 5. Minimal scope additions for safer prize fit

### ADD-01 - Agent venture overlay

- **Files:** `/demo-agents/research-agent/venture_profile.json`, `/demo-agents/research-agent/scenarios.json`
- **Acceptance:** demo vie vypisat venture profile, budget a policy hash pred payment flow.
- **Targets:** Umia, Network Economy.

### ADD-02 - FinTech risk panel

- **Files:** `/demo-scripts/sponsors/fintech-risk-panel.sh`, Web UI alebo CLI fallback.
- **Acceptance:** jeden screen ukaze risk decision, budget ledger, deny code, audit hash chain status.
- **Targets:** The FinTechers, Network Economy.

### ADD-03 - ENS agent identity proof

- **Files:** `/demo-scripts/sponsors/ens-agent-identity.sh`
- **Acceptance:** resolver alebo mocked resolver ukaze:
  - `sbo3l:agent_id=research-agent-01`
  - `sbo3l:policy_hash=0x...`
  - `sbo3l:endpoint=...`
- **Targets:** ENS AI Agents, Network Economy.

### ADD-04 - Hardware/privacy proof screen

- **Files:** `/demo-scripts/sponsors/hardware-privacy-proof.sh`
- **Acceptance:** screen ukaze signer backend, `key_exportable=false`, `agent_key_access=none`, kill switch status.
- **Targets:** Hardware Usage, Privacy by Design, FinTechers.

### ADD-05 - Optional Sourcify contract pre-check

- **Files:** `/demo-scripts/sponsors/sourcify-contract-check.sh`, later `/crates/sbo3l-onchain/src/contract_verification.rs`
- **Acceptance:** unknown/unverified contract call is denied before signing.
- **Targets:** Sourcify only.
- **Decision:** do only after Tier A/B demo is stable.

---

## 6. Do not expand into these before ETHPrague

- Full marketplace.
- ZK proofs.
- Hosted SaaS edition.
- Full production HSM-only mode if hardware blocks the demo.
- Full real TDX/SEV-SNP attestation if cloud/hardware setup is unstable.
- Multi-chain beyond Base Sepolia unless already working.

These are product-roadmap valid, but hackathon-risky.

---

## 7. Winning submission package

One repo submission should include:

- 3-minute video: real agent, legit payment, prompt-injection deny, audit, kill switch.
- 30-second Umia cut: agent venture budget + safe autonomous spend.
- 30-second FinTechers cut: risk/compliance/audit panel.
- ENS proof screenshot or script output.
- Hardware/privacy proof screenshot.
- README section: "Which prizes we target and why".

---

## 8. Recheck gate

Before final submission:

- Re-open Devfolio prize page.
- Confirm exact bounty names and judging criteria.
- Remove any conditional bounty not backed by a real runnable script.
- Do not mention SpaceComputer/Sourcify in final pitch unless the integration actually runs.

---

## 9. Win probability estimate

**Datum odhadu:** 2026-04-26  
**Caveat:** Toto nie je predikcia poroty. Je to prakticky odhad podla prize fitu, nasej dokumentacie, implementacnej narocnosti a pravdepodobnosti, ze demo bude posobit ako hotovy produkt.

### Scenare

| Scenario | Definicia |
|---|---|
| **Docs only** | Strategia a dokumentacia existuje, ale bez realneho runnable dema. |
| **Working build** | R1 funguje: agent, vault, policy, audit, x402/mock/live path. |
| **Winning demo** | Working build + real-agent prompt injection, venture overlay, risk panel, ENS proof, hardware/privacy proof, 3-min pitch nacviceny. |

### Individual reward chances

| Reward | Docs only | Working build | Winning demo | Verdikt |
|---|---:|---:|---:|---|
| **Umia - Best Agentic Venture** | 5-10% | 20-30% | **35-50%** | Najsilnejsi sponsor target, ak ukazeme agent venture loop. |
| **The FinTechers Track** | 5-10% | 20-35% | **35-45%** | Velmi silny fit, ak risk/compliance panel vyzera produkcne. |
| **ETHPrague Network Economy** | 4-8% | 18-30% | **30-40%** | Najlepsia hlavna cena; treba ukazat agent economy narrative. |
| **ENS - Best ENS Integration for AI Agents** | 2-5% | 10-20% | **25-35%** | Vyhratelne, ak ENS nie je iba dekoracia, ale identity/policy proof. |
| **Best Privacy by Design** | 5-10% | 18-30% | **30-40%** | Core architektura sedi; musi byt jasne vidiet no-key/no-seed boundary. |
| **Best Hardware Usage** | 2-5% | 10-20% | **25-35%** | Zavisi od fyzickeho kill switch/HSM momentu. SoftHSM same nestaci na top dojem. |
| **Best UX Flow** | 1-3% | 5-12% | 10-20% | Iba ak dashboard bude velmi cisty. Nie je primary target. |
| **Future Society** | 2-5% | 8-15% | 12-22% | Mozny secondary submit, ale narrative nie je najsilnejsi. |
| **SpaceComputer** | 0-3% | 5-12% | 18-30% | Len ak realne pouzijeme ich KMS/cTRNG/hardware API. |
| **Sourcify** | 0-2% | 5-10% | 12-22% | Len ak Sourcify data bude core komponent, nie bonus check. |
| **Ethereum Core** | <1% | <2% | <3% | Necielit. |
| **Swarm bounties** | <1% | 3-8% | 8-15% | Necielit, slaby strategic fit. |

### Portfolio chance

Ak dodame len dokumentaciu: **sanca vyhrat nieco je nizka, cca 5-10%**. Hackathon nevyhrava dokumentacia, ale runnable proof.

Ak dodame working R1 build bez skveleho stage dema: **sanca vyhrat aspon jednu relevantnu cenu je cca 35-50%**.

Ak dodame winning demo podla tohto dokumentu:

- **aspon jedna relevantna cena:** 60-75%
- **dve relevantne ceny:** 30-45%
- **tri a viac cien:** 12-25%

Najrealistickejsia multi-win kombinacia:

1. **Umia Best Agentic Venture**
2. **The FinTechers Track**
3. **Network Economy** alebo **ENS AI Agents**
4. bonus: **Privacy by Design** alebo **Hardware Usage**

### Co najviac zvysi sance

1. Real-agent demo musi byt skutocne zive, nie curl.
2. Legit spend aj malicious spend musia ist cez rovnaky agent/vault path.
3. UI/CLI musi ukazat presny deny code, policy hash, budget a audit chain.
4. Agent venture overlay musi byt zrozumitelny za 20 sekund.
5. Hardware/privacy proof musi byt vizualny: key not exportable, agent key access none, kill switch active.
6. Final pitch musi mat jednu vetu: **Don't give your agent a wallet. Give it a mandate.**

---

## 10. Deep strategic revision: from vault to Agent Venture Firewall

### Finding

Po re-checku ETHPrague/Devfolio, Umia, x402, AP2, ENSIP-25, Sourcify a SpaceComputer je najvacsi pattern:

- ETHPrague main tracks hladaju real-world impact, user control, privacy, identity a on-chain economic systems.
- Umia nehlada iba "agent tool"; hlada **agentic venture**: funded, governed, treasury-managed agent business.
- The FinTechers hladaju deployable finance products: payments, audit, risk, compliance, guardrails.
- ENS AI Agents hladaju verifiable agent identity.
- x402/AP2 trend hovori: agents will pay, but trust/authorization/accountability is the open problem.

Preto by SBO3L nemal byt pitchovany iba ako:

> local signer for AI agents

ale ako:

> **Agent Venture Firewall** - the spend-control, identity, audit and signer layer for autonomous agent businesses.

Toto nie je zmena jadra produktu. Je to presnejsi packaging + maly scope overlay.

### New winning product frame

**Old frame:** Agent nema wallet, ma vault.  
**Better frame for prizes:** Agentic ventures need a CFO, auditor and security officer before they can safely hold treasury.

Najlepsia veta:

> SBO3L turns an AI agent from a hot-wallet bot into a policy-bound, auditable economic actor.

Slovensky:

> SBO3L spravi z AI agenta ekonomickeho aktora, ale nie nebezpecnu hot wallet.

---

## 11. Recommended scope upgrade for maximum win chance

### Upgrade A - Agent Venture Passport

Pridat jeden normativny objekt:

```json
{
  "venture_id": "research-agent.team.eth",
  "agent_id": "research-agent-01",
  "purpose": "Autonomous research agent that buys paid data/API calls and produces reports",
  "treasury_policy": {
    "daily_cap_usd": "10.00",
    "per_tx_cap_usd": "0.50",
    "allowed_protocols": ["x402"],
    "allowed_providers": ["api.example.com"],
    "allowed_recipients": ["0x1111111111111111111111111111111111111111"]
  },
  "identity": {
    "ens": "research-agent.team.eth",
    "ensip25": true
  },
  "audit": {
    "latest_policy_hash": "0x...",
    "latest_audit_root": "0x..."
  }
}
```

**Preco:**  
Toto spoji Umia, Network Economy, ENS, FinTechers a Privacy do jedneho artefaktu. Porota uvidi, ze nejde iba o payment request, ale o economic identity + treasury governance.

**Scope impact:** low. Je to JSON + UI/CLI render + demo fixture.

### Upgrade B - Agent Venture Lifecycle demo

Demo uz nema byt iba "request allow/deny". Ma mat 4 akty:

1. **Register identity:** agent ma ENS/venture passport.
2. **Fund budget:** venture dostane daily treasury budget.
3. **Earn/build:** agent kupi paid API cez x402 a vytvori output.
4. **Survive attack:** prompt injection chce malicious spend, vault deny + audit proof.

**Preco:**  
Umia hovorí o launching/funding/governing agentic ventures. Network Economy hovorí o on-chain economic systems. Toto presne ukaze cely mini economy loop.

**Scope impact:** medium-low. Vacsinou demo orchestration.

### Upgrade C - Public Trust Badge

Pridat public read-only view:

```text
research-agent.team.eth
status: policy-bound
vault: self-hosted
private_key_exposure: none
daily_budget: $10
last_audit_root: 0x...
latest_policy_hash: 0x...
ens_verified: yes
```

**Preco:**  
Judges a sponzori okamzite pochopia: toto je trust layer pre agent economy. Je to aj silny UX moment.

**Scope impact:** low. Web/CLI/HTML page staci.

### Upgrade D - Policy receipts as proof of authorization

Kazda platba/deny event generuje receipt:

```json
{
  "receipt_type": "sbo3l.policy_receipt.v1",
  "agent_id": "research-agent-01",
  "decision": "deny",
  "deny_code": "policy.deny_recipient_not_allowlisted",
  "request_hash": "0x...",
  "policy_hash": "0x...",
  "audit_event": "evt-...",
  "signature": "ed25519:..."
}
```

**Preco:**  
AP2/x402 trend je o authorization/accountability. Toto je nas moat: agent payment proof + policy proof + audit proof.

**Scope impact:** medium. Ale mame uz decision token/audit event, tak je to iba public wrapper.

### Upgrade E - Sponsor-specific cuts from same demo

Jedno demo, viac renderov:

- **Umia cut:** agent venture budget + safe autonomous spend.
- **FinTechers cut:** risk decision + audit + compliance export.
- **Network Economy cut:** agent as economic actor with user-controlled treasury.
- **ENS cut:** verified agent identity via ENSIP-25 style record.
- **Hardware/privacy cut:** key never leaves vault, kill switch works.

**Preco:**  
Netreba viac produktov. Treba viac uhlov pohladu na jednu fungujucu vec.

---

## 12. Revised probability after scope upgrade

Ak pridame Agent Venture Passport + lifecycle demo + trust badge + policy receipts, odhad sa zlepsi:

| Reward | Povodny winning demo | Revised winning demo |
|---|---:|---:|
| **Umia - Best Agentic Venture** | 35-50% | **45-60%** |
| **The FinTechers Track** | 35-45% | **40-55%** |
| **ETHPrague Network Economy** | 30-40% | **38-50%** |
| **ENS AI Agents** | 25-35% | **35-45%** |
| **Privacy by Design** | 30-40% | **35-45%** |
| **Hardware Usage** | 25-35% | **25-40%** |
| **Best UX Flow** | 10-20% | **18-30%** |

Portfolio:

- **aspon jedna relevantna cena:** 70-85%
- **dve relevantne ceny:** 40-60%
- **tri a viac cien:** 20-35%

Toto je optimisticky, ale nie fantazijny, ak build a demo budu naozaj fungovat.

---

## 13. What not to do

Nezvacsovat scope tymto smerom:

- full Umia clone,
- full marketplace,
- vlastny launchpad,
- vlastny ENS registry,
- vlastny AP2/x402 replacement,
- ZK policy proof,
- complex multi-chain treasury.

Tieto veci zneistia porotu a rozbiju demo. Najlepsi tah je: **one agent, one venture, one vault, one policy, one attack, one proof.**

---

## 14. Final recommendation

Projekt nemenit z SBO3L na nieco ine. Rozsirit ho takto:

> SBO3L is the Agent Venture Firewall: a local, policy-bound payment vault that lets autonomous AI ventures spend, earn and prove compliance without ever holding their own private keys.

To je najvyssia sanca na multi-prize win, lebo jeden produkt naraz trafia:

- Umia agentic venture,
- Network Economy,
- FinTechers risk/compliance,
- ENS agent identity,
- Privacy by Design,
- Hardware Usage.
