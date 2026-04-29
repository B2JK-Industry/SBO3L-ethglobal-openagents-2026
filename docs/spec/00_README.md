# SBO3L

**Názov produktu / public brand:** **SBO3L** (rozhodnuté 2026-04-27, viď `OQ-15 UPDATED` + `19_knowledge_base.md §11`)
**Tagline:** **Spending mandates for autonomous agents.**
**Pitch veta:** **Don't give your agent a wallet. Give it a mandate.**
**Predchádzajúce pracovné názvy:** Agent Vault OS, Local Agent Trezor, Agent Payment Coprocessor (zahodené po naming research-u — kolízie s `cloudweaver/agentvault`, `Infisical/agent-vault`, ThoughtMachine VaultOS, Trezor brand)
**Technical namespace / daemon namespace:** `mandate` (crates, cesty, schema IDs, CLI a interný daemon)
**Status:** Pre-implementation documentation + machine-readable contracts complete; ready for paralelnú implementáciu cez AI agentov
**Dátum poslednej revízie:** 2026-04-27
**Owner:** Daniel Babjak

---

## Hlavná téza

> **AI agent nesmie mať wallet. AI agent musí dostať mandát: obmedzené, auditovateľné oprávnenie konať.**

SBO3L je lokálny bezpečnostný produkt na vlastnom Linux servery, ktorý umožňuje AI agentom vykonávať autonómne mikroplatby a on-chain akcie (x402, stablecoin, API payments), bez toho aby agent vlastnil alebo videl private key — a zároveň bez nutnosti manuálneho potvrdzovania každej transakcie ako pri klasickom Trezore.

**Brand vs implementation:** V pitchi, submission, UI aj kóde používame jednotne **SBO3L** / `mandate`: crate prefix, daemon, CLI, filesystem path aj schema host.

---

## Status balíka

✅ **Komplet pre-implementation balík dokumentov.** 34 top-level očíslovaných dokumentov pokrýva strategickú víziu, threat model, architektúru, contracts, demo acceptance harness, knowledge base, install + demo procedures, backlog, agent assignment matrix, review, market research, hackathon winning demo plan, bounty strategy, ETHGlobal Open Agents pivot, two-developer execution plan, ETHGlobal submission compliance, implementation safeguards, end-to-end implementation spec, Claude Code prompty pre oboch techlead developerov a initial orchestrator prompt.

Produktový kód ešte nie je napísaný. Repo už ale obsahuje normatívne JSON schémy, OpenAPI draft, golden/adversarial corpus a real-agent demo harness spec, takže implementácia sa dá rozdeliť medzi viacerých AI agentov bez hádania wire formátov.

---

## Kompletná štruktúra dokumentácie

### Strategy & Vision
| # | Súbor | Sekcia/účel |
|---|---|---|
| 00 | [`README.md`](00_README.md) | Tento súbor — index a navigácia |
| 01 | [`executive_summary.md`](01_executive_summary.md) | Executive summary (10–15 viet) |
| 02 | [`market_analysis.md`](02_market_analysis.md) | Trh a porovnávacia tabuľka (15+ riešení) |
| 03 | [`positioning.md`](03_positioning.md) | Product positioning + 3 positioning vety |
| 24 | [`deep_market_research_2026.md`](24_deep_market_research_2026.md) | Hlboký trhový prieskum 2026 + konkurencia + budúci potenciál |

### Security & Architecture
| # | Súbor | Sekcia/účel |
|---|---|---|
| 04 | [`threat_model.md`](04_threat_model.md) | 25 útokov + STRIDE + invarianty |
| 05 | [`trust_boundaries.md`](05_trust_boundaries.md) | 6 zón + capability model |
| 06 | [`reference_architecture.md`](06_reference_architecture.md) | 4 varianty (V1 MVP → V4 cieľový TEE+HSM) |
| 07 | [`unique_architecture.md`](07_unique_architecture.md) | 10 stavebných blokov |

### Implementation Spec
| # | Súbor | Sekcia/účel |
|---|---|---|
| 08 | [`data_flow.md`](08_data_flow.md) | 14-krokový payment flow + ASCII diagram + sequence |
| 09 | [`api_design.md`](09_api_design.md) | 16+ REST endpointov (request/response/audit events) |
| 10 | [`data_model.md`](10_data_model.md) | 21 entít + ER diagram |
| 11 | [`policy_model.md`](11_policy_model.md) | Policy DSL, YAML examples, Rego compilation |
| 26 | [`end_to_end_implementation_spec.md`](26_end_to_end_implementation_spec.md) | **Build order + production readiness checklist** |

### Backlog & Acceptance
| # | Súbor | Sekcia/účel |
|---|---|---|
| 12 | [`backlog.md`](12_backlog.md) | **Phased backlog**: 18 epicov, 113 stories, 11 phases |
| 13 | [`mvp_scope.md`](13_mvp_scope.md) | 4 release miľníky (R1-R4) → phase mapping |
| 14 | [`open_questions.md`](14_open_questions.md) | 17 otvorených otázok + 10 rizík + decisions log |

### Hackathon-specific
| # | Súbor | Sekcia/účel |
|---|---|---|
| 15 | [`review_ethprague.md`](15_review_ethprague.md) | Objektívna recenzia projektu pre ETHPrague |
| 16 | [`demo_acceptance.md`](16_demo_acceptance.md) | **129 primary phase gates + red-team scenarios** |
| 25 | [`ethprague_sponsor_winning_demo.md`](25_ethprague_sponsor_winning_demo.md) | Finálny sponsor-winning demo plan s reálnym agentom |
| 27 | [`ethprague_bounty_strategy.md`](27_ethprague_bounty_strategy.md) | **Cieľové bounty, no-go bounty a scope-fit verdict** |
| 28 | [`ethglobal_openagents_pivot.md`](28_ethglobal_openagents_pivot.md) | **Pivot analýza pre ETHGlobal Open Agents / agent hackathony** |
| 29 | [`two_developer_execution_plan.md`](29_two_developer_execution_plan.md) | **Presný plán práce pre dvoch developerov** |
| 30 | [`ethglobal_submission_compliance.md`](30_ethglobal_submission_compliance.md) | **ETHGlobal rules, AI usage, partner prizes a demo video checklist** |
| 31 | [`claude_code_techlead_prompt.md`](31_claude_code_techlead_prompt.md) | **Copy-paste prompt pre prvého Claude Code techlead developera** |
| 32 | [`claude_code_second_techlead_prompt.md`](32_claude_code_second_techlead_prompt.md) | **Copy-paste prompt pre druhého Claude Code techlead developera** |
| 33 | [`claude_code_initial_orchestrator_prompt.md`](33_claude_code_initial_orchestrator_prompt.md) | **Initial prompt pre Claude Code: repo, implementácia, PR, Codex review** |

### Parallel-Implementation Infrastructure
| # | Súbor | Sekcia/účel |
|---|---|---|
| 17 | [`interface_contracts.md`](17_interface_contracts.md) | **Locked schemas/contracts** (single source of truth) |
| 18 | [`agent_assignment.md`](18_agent_assignment.md) | **Agent slot matrix** pre paralelnú prácu (40 slotov) |
| 19 | [`knowledge_base.md`](19_knowledge_base.md) | **Compiled deep research KB** (TEE, x402, Rust, hardening) |

### Machine-Readable Implementation Artifacts
| Path | Účel |
|---|---|
| [`schemas/`](schemas/) | JSON Schema 2020-12 pre APRP, policy, x402, audit event a decision token |
| [`docs/api/openapi.json`](docs/api/openapi.json) | OpenAPI 3.1 draft pre REST API + SDK generation |
| [`test-corpus/`](test-corpus/) | Golden/adversarial fixtures pre contract tests |
| [`demo-agents/research-agent/`](demo-agents/research-agent/) | Real-agent ETHPrague demo harness contract |

### Operations
| # | Súbor | Sekcia/účel |
|---|---|---|
| 20 | [`linux_server_install.md`](20_linux_server_install.md) | Step-by-step install na Ubuntu 24.04 |
| 21 | [`demo_setup.md`](21_demo_setup.md) | Demo logistika (HW shopping list, pitch flow, fallbacks) |

### Quality / Anti-Bug
| # | Súbor | Sekcia/účel |
|---|---|---|
| 22 | [`backlog_review.md`](22_backlog_review.md) | Correctness review backlog-u (7 critical fixes applied) |
| 23 | [`implementation_safeguards.md`](23_implementation_safeguards.md) | Anti-bug compendium (200+ konkrétnych gotchas) |

---

## Tri hlavné architektonické tézy

1. **Separation of concerns medzi rozhodnutím a podpisom.** Agent vie iba *žiadať*. Policy engine *rozhoduje*. Signer iba *podpisuje schválené*. Tieto tri vrstvy nesmú zdieľať trust doménu.
2. **Policy-as-code, podpísané a versionované.** Žiadny agent ani admin proces nesmie zmeniť policy bez ľudského schválenia + auditovaného hash-chained zápisu.
3. **Attestable runtime.** Cieľová verzia (Variant 4) beží v TEE a generuje attestation evidence — externá strana vie overiť, že podpis vznikol cez nepozmenený vault runtime nad nepozmenenými pravidlami.

---

## Phase mapa (z backlogu)

| Phase | Hlavná téza | Demo gates |
|---|---|---|
| **P0** Foundations | Repo, CI, telemetry, contracts locked | 5 |
| **P1** Happy-path single payment | Agent → vault → encrypted-file sign → x402 mock → return | 13 |
| **P2** Real policy + budget | Rego eval, multi-scope budget, deny rules | 18 |
| **P3** Audit + emergency | Hash chain, signed events, freeze, kill switch | 14 |
| **P4** Real x402 + simulator | Live Base testnet, transaction simulation, RPC quorum | 11 |
| **P5** HW isolation | YubiHSM/Nitrokey + TPM 2.0; encrypted-file deprecated for prod | 10 |
| **P6** Approval + governance UI | Web UI + push relay; M-of-N admin signatures | 14 |
| **P7** TEE + attestation | TDX/SEV-SNP confidential VM; HW-rooted attestation | 12 |
| **P8** **On-chain (HACKATHON HERO)** | Smart account session keys; on-chain attestation verifier | 14 |
| **P9** Marketplace + advanced | MCP server, marketplace pilot, ZK proof (stretch) | 10 |
| **P10** Polish + release | Appliance image, hardening, security audit | 8 |

**Total primary phase gates:** 129. Full acceptance surface má 148 runnable scenárov vrátane Open Agents overlay, phase red-team a final red-team hardening scenárov. Každý critical gate musí passnúť pred ďalším phase.

---

## Quick links — ako čítať tento balík

### Pre produktového manažéra / investora
1. [`01_executive_summary.md`](01_executive_summary.md) — čo to je
2. [`03_positioning.md`](03_positioning.md) — pre koho a čo
3. [`13_mvp_scope.md`](13_mvp_scope.md) — kedy
4. [`15_review_ethprague.md`](15_review_ethprague.md) — kde sa to bude prezentovať

### Pre security architekta
1. [`04_threat_model.md`](04_threat_model.md) — 25 útokov
2. [`05_trust_boundaries.md`](05_trust_boundaries.md) — zóny
3. [`07_unique_architecture.md`](07_unique_architecture.md) — design
4. [`19_knowledge_base.md`](19_knowledge_base.md) §1, §6, §7 — TEE + Linux + prompt injection

### Pre backend / infra inžiniera (alebo AI agenta v loope)
1. [`17_interface_contracts.md`](17_interface_contracts.md) — locked contracts (MUSÍŠ poznať)
2. [`26_end_to_end_implementation_spec.md`](26_end_to_end_implementation_spec.md) — build order + production checklist
3. [`19_knowledge_base.md`](19_knowledge_base.md) §5 — Rust crate stack
4. [`12_backlog.md`](12_backlog.md) — story zadania
5. [`18_agent_assignment.md`](18_agent_assignment.md) — môj slot
6. [`23_implementation_safeguards.md`](23_implementation_safeguards.md) — anti-bug guide
7. [`16_demo_acceptance.md`](16_demo_acceptance.md) — acceptance gate

### Pre on-chain / smart contract inžiniera
1. [`19_knowledge_base.md`](19_knowledge_base.md) §4 — ERC-4337 + Safe + on-chain DCAP
2. [`12_backlog.md`](12_backlog.md) Phase 8 — on-chain stories
3. [`16_demo_acceptance.md`](16_demo_acceptance.md) Phase 8 demos

### Pre DevOps / SRE
1. [`20_linux_server_install.md`](20_linux_server_install.md) — install procedure
2. [`19_knowledge_base.md`](19_knowledge_base.md) §6 — Linux hardening
3. [`13_mvp_scope.md`](13_mvp_scope.md) — release matrix

### Pre orchestrátora paralelných agentov
1. [`18_agent_assignment.md`](18_agent_assignment.md) — mapovanie slotov
2. [`12_backlog.md`](12_backlog.md) §15 — dependency graph
3. [`16_demo_acceptance.md`](16_demo_acceptance.md) — loop runner contract

### Pre demo presentation tím
1. [`21_demo_setup.md`](21_demo_setup.md) — HW shopping + stage layout
2. [`28_ethglobal_openagents_pivot.md`](28_ethglobal_openagents_pivot.md) — primary Open Agents package
3. [`25_ethprague_sponsor_winning_demo.md`](25_ethprague_sponsor_winning_demo.md) — secondary ETHPrague narrative
4. [`16_demo_acceptance.md`](16_demo_acceptance.md) red-team demos

### Pre dvoch developerov
1. [`30_ethglobal_submission_compliance.md`](30_ethglobal_submission_compliance.md) — pravidla hackathonu a submission compliance
2. [`29_two_developer_execution_plan.md`](29_two_developer_execution_plan.md) — kto robi co
3. [`26_end_to_end_implementation_spec.md`](26_end_to_end_implementation_spec.md) — build order
4. [`17_interface_contracts.md`](17_interface_contracts.md) — wire contracts
5. [`12_backlog.md`](12_backlog.md) — stories
6. [`16_demo_acceptance.md`](16_demo_acceptance.md) — acceptance gates

---

## Loop spawn template (pre orchestrátora)

Keď spustíš implementačný loop, každý agent dostáva:

```
Agent(
  description: "SLOT-PN-X: <slot-name>",
  subagent_type: "general-purpose",
  prompt: """
  You are implementing SLOT-PN-X of the SBO3L project (technical namespace: mandate).

  ## Pre-read (mandatory, in order)
  1. /Users/danielbabjak/Desktop/agent-vault-os/17_interface_contracts.md
  2. /Users/danielbabjak/Desktop/agent-vault-os/26_end_to_end_implementation_spec.md
  3. /Users/danielbabjak/Desktop/agent-vault-os/19_knowledge_base.md (sections from your slot brief)
  4. /Users/danielbabjak/Desktop/agent-vault-os/23_implementation_safeguards.md (sections U-* + your phase)
  5. /Users/danielbabjak/Desktop/agent-vault-os/16_demo_acceptance.md (your D-PN-* demos)
  6. /Users/danielbabjak/Desktop/agent-vault-os/12_backlog.md (your story)

  ## Stories to implement
  <list from 12_backlog.md>

  ## Allowed modules
  <list — do NOT touch other files>

  ## Acceptance gates (MUST PASS)
  bash demo-scripts/run-single.sh D-PN-NN ...

  ## Constraints
  - Follow §10 forbidden patterns in 17_interface_contracts.md
  - Use only crates listed in 19_knowledge_base.md §5
  - Hand back a summary: what was implemented, demos passed, any deviations

  ## Failure handling
  If a demo fails, fix the implementation (NOT the demo). If you believe demo is wrong, escalate — do not modify demo scripts.
  """,
  run_in_background: true
)
```

---

## Decisions log (TL;DR z `19_knowledge_base.md §11`)

| Decision | Choice |
|---|---|
| Implementation language | Rust |
| Policy DSL | Rego via `regorus` |
| JSON canonicalization | `serde_json_canonicalizer` |
| secp256k1 | `k256` |
| Storage | `rusqlite` + WAL |
| Money type | `rust_decimal` |
| Ethereum stack | `alloy` v1.x |
| TEE platform (primary) | Intel TDX |
| TEE platform (secondary) | AMD SEV-SNP |
| HSM (primary) | Nitrokey HSM 2 |
| HSM (secondary) | YubiHSM 2 |
| Payment protocol | x402 v2 (primary), l402 (secondary) |
| Smart account standard | ERC-7579 |
| On-chain DCAP verifier | Automata DCAP v1.1 |
| Chain (primary) | Base |
| Chain (secondary) | Polygon, Arbitrum |
| Bundler (4337) | Pimlico Alto |
| Code signing | cosign v3 + SLSA L3 |
| Linux base | Ubuntu 24.04 LTS |

---

## Contact / Maintenance

- **Owner:** Daniel Babjak (`babjak_daniel@hotmail.com`)
- **Project root:** `/Users/danielbabjak/Desktop/agent-vault-os/`
- **External memory:** legacy local planning memory exists outside this repo; these repository docs are the source of truth.

Pri zmene contractov / KB / safeguards always update všetky dotknuté súbory v jednom paneli, aby cross-document consistency zostala (per `22_backlog_review.md §6`).
