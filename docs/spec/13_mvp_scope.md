# Scope & Milestones

> **Filozofia:** Time je relaxed (per user feedback `feedback_no_time_objections`). Phases sú **value-based**, nie time-based — viď `12_backlog.md`. Tento dokument mapuje phases na **release miľníky** pre external pozorovateľov (sponsorov, partnerov).

---

## §1 Release miľníky vs phases

| Release | Includes phases | Externe sa dá popísať ako |
|---|---|---|
| **R1 — Hackathon Demo** | P0–P4 + Open Agents adapters + P8 selectively | "SBO3L: Open Agent Payment Firewall s policy receipts, ENS identity a sponsor-native execution adaptermi" |
| **R2 — HSM Production** | P5 + P6 | "Production-grade vault s HSM + multi-admin governance" |
| **R3 — Sovereign TEE** | P7 + P8 + P9 | "Plne sovereign vault s TEE attestation + on-chain verifier + marketplace" |
| **R4 — v1.0 Release** | P10 | "Open source v1.0 s externý security audit + appliance image" |

Neexistuje "M1/M2/M3" v zmysle 3-mesiacov / 6-mesiacov / 12-mesiacov — namiesto toho každý release ide vlastným tempom, viď phase exit criteria v `12_backlog.md`.

---

## §2 R1 — Hackathon Demo Release

### §2.1 Cieľ
Mať fungujúci, demonštrovateľný, end-to-end vault pre **ETHGlobal Open Agents ako primary target** a ETHPrague ako secondary package:
- Agent SDK (Python) + vault daemon
- Real or mocked x402/payment action path
- Live red-team scenarios (prompt injection, kill switch, tampering detect)
- Policy receipts for allow/deny
- ENS identity proof for AI agent
- KeeperHub guarded execution adapter
- Uniswap guarded swap adapter if time permits
- Agent venture overlay pre ETHPrague/Umia secondary package: venture profile, budget, policy hash, ENS identity

### §2.2 Scope
Phases: **P0, P1, P2, P3** plnohodnotne + vybrane **P4** payment/simulation features + **Open Agents sponsor adapters** + selektívne **P8** only if needed for sponsor proof.

Konkrétne stories required:
- Foundations (E1)
- APRP + Python SDK (E2-S1, S2, S3)
- Gateway + mTLS + rate limiting (E3-S1, S2, S4)
- Policy engine + budget ledger (E4, E5)
- x402 verifier + simulator (E6, E7)
- Audit log + emergency controls (E10-S1,S2; E12)
- Encrypted file signing + decision token (E8-S1, S5)
- Real or mocked x402/payment path (E16-S1 if live chain is stable)
- Policy receipt wrapper around decision token + audit event
- Open Agents sponsor scripts:
  - ENS identity proof
  - KeeperHub guarded execution
  - Uniswap guarded swap if time permits
- Real-agent harness + live attack demos (E11-S8)
- Bounty overlay z `27_ethprague_bounty_strategy.md`: agent venture profile, FinTech risk panel, ENS identity proof, hardware/privacy proof screen
- Custom 4337 attested validator (E16-S6) — ETHPrague optional hero, not Open Agents blocker
- On-chain audit anchor (E16-S7) — optional

### §2.3 Out of scope (R1)
- HSM (encrypted file backend stačí pre demo)
- Real TDX/SEV-SNP attestation (self-signed attestation OK)
- Multi-admin governance UI (CLI stačí)
- Mobile PWA
- Marketplace pilot
- ZK proofs
- SpaceComputer/Sourcify integracie, pokial nie je core Tier A/B demo uz stabilne
- Full 4337/on-chain attestation if Open Agents timeline is tight
- Gensyn AXL and 0G deep integrations unless KeeperHub + ENS are already stable

### §2.4 Demo gate
Pre Open Agents musia passnut primary `D-P0-*`, `D-P1-*`, `D-P2-*`, `D-P3-*`, real-agent red-team `D-P8-11`, tamper/kill switch `D-P8-12..13`, plus Open Agents sponsor demos definovane v `28_ethglobal_openagents_pivot.md` a `29_two_developer_execution_plan.md`.

Pre ETHPrague secondary package navyse passnut selektívne `D-P4-*` live payment gates a `D-P8-03..05`, `D-P8-08..14` podla aktualneho sponsor scope.

### §2.5 Sponsor target
- **Primary hackathon package:** ETHGlobal Open Agents.
- **Primary Open Agents sponsor prizes:** KeeperHub, ENS AI Agents.
- **Secondary Open Agents sponsor prizes:** Uniswap, Gensyn AXL, 0G only if runnable.
- **Secondary ETHPrague package:** Network Economy, Umia, The FinTechers, Privacy by Design, Hardware Usage.

---

## §3 R2 — HSM Production Release

### §3.1 Cieľ
Použiteľné v reálnej výrobe pre tímy s HSM/TPM.

### §3.2 Scope
Phases: **P5, P6** + production hardening.

Pridáva oproti R1:
- PKCS#11 backend (YubiHSM 2 + Nitrokey HSM 2 + SoftHSM CI)
- TPM 2.0 backend
- Production config validator (`mandate config check --production`)
- Admin enrollment + M-of-N multisig
- Web UI pre approvals
- Push notification cez vlastný relay
- RBAC
- Reference policy library
- Hardening guide (AppArmor/SELinux profiles)
- LangChain/AutoGen/MCP integration cookbooks

### §3.3 Demo gate
Všetky `D-P5-*` + `D-P6-*` musia passnúť.

### §3.4 Out of scope (R2)
- TEE
- On-chain integrácia (zostáva zo R1)
- Marketplace
- Externý audit

---

## §4 R3 — Sovereign TEE Release

### §4.1 Cieľ
Cieľová bezpečnostná postura. Vault beží v TDX/SEV-SNP, attestation evidence on-chain verifikovaná, smart accounty integrované.

### §4.2 Scope
Phases: **P7, P8, P9** plnohodnotne.

Pridáva:
- Self-signed attestation (P7-A) ako baseline
- Intel TDX attestation cez `dcap-qvl` + configfs-tsm
- AMD SEV-SNP attestation
- TEE-sealed signing backend (KMS-as-TApp pattern)
- Attestation drift detection + auto-freeze
- Safe attested module
- Custom 4337 validator s on-chain DCAP (full integration)
- On-chain audit anchor + policy registry
- ENS subnames pre agentov
- EAS / Verax attestation publishing
- Marketplace pilot
- ZK proof of policy eval (stretch)
- Mobile PWA
- Static binary releases s SLSA L3
- `.deb`/`.rpm` packages
- Docker compose example
- Reproducible build verification

### §4.3 Demo gate
Všetky `D-P7-*`, `D-P8-*`, `D-P9-*` musia passnúť.

---

## §5 R4 — v1.0 Release

### §5.1 Cieľ
Open source v1.0.0 s externý security audit dokončený.

### §5.2 Scope
Phase: **P10**.

Pridáva:
- Appliance image (bootovateľný USB)
- Helm chart pre k8s
- Externý security audit (Trail of Bits / Zellic / Cure53)
- Editions matrix (community vs enterprise)
- Pricing model + LOI
- Production runbooks
- API reference docs
- Public release announcement

### §5.3 Demo gate
Všetky `D-P10-*` + cumulative re-run všetkých P0-P9 demos.

---

## §6 Phase → Release table

| Phase | R1 | R2 | R3 | R4 |
|---|---|---|---|---|
| P0 Foundations | ✅ | — | — | — |
| P1 Happy path | ✅ | — | — | — |
| P2 Policy + budget | ✅ | — | — | — |
| P3 Audit + emergency | ✅ | — | — | — |
| P4 Real x402 + sim | ✅ | — | — | — |
| P5 HW isolation | — | ✅ | — | — |
| P6 Approval + governance | — | ✅ | — | — |
| P7 TEE + attestation | — | — | ✅ | — |
| P8 On-chain integration | partial (sponsor demos) | — | ✅ full | — |
| P9 Marketplace + advanced | — | — | ✅ | — |
| P10 Polish + release | — | — | — | ✅ |

---

## §7 Tím (predpokladaný, nie viazaný na čas)

| Rola | R1 | R2 | R3 | R4 |
|---|---|---|---|---|
| Backend Rust eng. | 2 | 2 | 3 | 1 |
| Security / crypto eng. | 1 | 1 | 2 | 1 |
| TEE / HSM eng. | 0.5 | 1 | 2 | 0 |
| Smart contract eng. (Solidity) | 1 | 0 | 1 | 0 |
| Frontend (web UI / PWA) | 0.5 | 1 | 1 | 0.5 |
| DevOps / packaging | 0.5 | 0.5 | 1 | 1 |
| Product / docs | 0.5 | 0.5 | 1 | 1 |

S paralelizáciou cez AI agentov (per `18_agent_assignment.md`) sa to dá scalovať up alebo down.

---

## §8 Critical decision miľníky (gate checks)

### Pre-R1 (sa rozhoduje teraz)
- Voľba HSM vendor pre R2 (Nitrokey HSM 2 vs YubiHSM 2 — viď OQ-06).
- Marketing meno: **SBO3L** public brand, `mandate` technical namespace — viď OQ-15.
- Decision: pôjde R1 na live mainnet alebo iba testnet? **Návrh: testnet pre demo, mainnet ready pre R2.**

### Post-R1 (po hackathone)
- Sponsor feedback inkorporácia.
- Decision: zacieliť na production-HSM segment (R2) alebo skočiť priamo na TEE (R3)? **Návrh: R2 najprv** — širší user base, validuje celý policy/audit/governance stack.

### Pre-R2
- Voľba HSM models (final test — buy oba, otestovať production stability).
- Pricing of community vs enterprise edition.

### Pre-R3
- TEE platform priority — TDX vs SEV-SNP. **Návrh: TDX primary** (better gas costs on-chain, growing HW availability).
- Smart account architecture — vlastný validator vs Safe modul priority. **Návrh: oboje, ale Safe modul najprv** (širšia adopcia).

### Pre-R4
- Audit firm selection.
- Open source license confirm (Apache 2.0).
- Funding strategy (open source vs hosted enterprise).

---

## §9 Risks per release

### R1 risks
- Demo-day technical failure → mitigované per `21_demo_setup.md §6`.
- Konkurencia v field-e (predikujem 8-15 podobných tímov) → mitigované cez differentiator (TEE-attested 4337 validator).

### R2 risks
- HSM supply chain (Nitrokey HSM 2 dodávky) — mitigované dual-vendor support.
- Adopcia closed-source HSM users vs open-source preference.

### R3 risks
- TDX hardware availability pre home server (Granite Rapids-WS až 2026 LGA 4710).
- TEE side-channel discoveries (TEE.fail trieda) — vyžaduje rapid security update flow.
- On-chain DCAP gas cost volatility.

### R4 risks
- Externý audit findings (môžu posunúť release).
- Open source community adoption uncertainty.
