# Open Questions & Risks

## OQ — Otvorené otázky

Označenie: **OQ-XX** + impact (M1/M2/M3) + owner (TBD).

---

### OQ-01 (M1) Voľba implementačného jazyka — Rust vs Go
- **Stav:** Rust preferovaný (memory safety, single static binary, výborné crypto crates), ale Go má rýchlejší developer ramp-up a väčší pool.
- **Decision needed by:** mesiac 1.
- **Trade-off:** Rust = bezpečnosť, ale vyšší cost developmentu; Go = rýchlosť developmentu, ale menej safety nad raw bytes.
- **Návrh:** Rust pre core (validator, policy engine, signer adapter), TypeScript/Python iba pre client SDKs.

### OQ-02 (M1) Voľba policy DSL — Rego vs CEL vs vlastné
- **Rego (OPA):** mature, expressive, ale vyžaduje OPA runtime alebo embedded eval (`regorus`).
- **CEL:** ľahší, používaný v K8s admission, dobré tooling, ale menej výrazný.
- **Vlastné:** maximum control, ale risk re-implementácie zranitelností.
- **Návrh:** Rego cez `regorus` (Rust embedded). Re-evaluation po PoC.

### OQ-03 (M1) Storage — SQLite vs embedded Postgres vs sled/RocksDB
- SQLite je single-file, atomic, well-understood. Pre M1 jasne víťaz.
- Pre M3 by sa mohol zísť Postgres pre HA (multi-vault setup) — open question.

### OQ-04 (M1) Identity model agenta — mTLS vs JWT vs OAuth client_credentials
- mTLS najsilnejšie, ale provisioning je composability pain (cert rotation pre stovky agentov).
- JWT s krátkou TTL + refresh by mohol byť pragmatickejší.
- **Návrh:** mTLS default, JWT opt-in (E3-S3).

### OQ-05 (M1) HTTP 402 / x402 versioning — ktoré verzie podporujeme
- Coinbase x402 spec aktuálne aktívne vyvíjaný. Sledovať RFC.
- l402 (Lightning Labs) je príbuzný protokol — rovnako podporovať?
- **Návrh:** v1 podporuje x402, l402 ako E16-S4 alebo neskôr.

### OQ-06 (M2) HSM vendor — YubiHSM 2 vs Nitrokey HSM 2 vs OpenPGP smartcard
- YubiHSM 2: $650, vendor-locked SDK, dobré attestation, USB.
- Nitrokey HSM 2: ~€100, open-source firmware, PKCS#11, USB.
- **Návrh:** podporovať oba cez PKCS#11 (kompromis); pre dev SoftHSM.

### OQ-07 (M3) TEE platforma — TDX vs SEV-SNP vs Nitro vs SGX
- TDX: Intel, novšie (4th-gen Xeon, niektoré 12th-gen+ konzumér), VM-based.
- SEV-SNP: AMD, mature, EPYC + Hetzner cloud SKUs.
- Nitro Enclaves: AWS-only, ale výborná attestation pipeline.
- SGX: starší, problematický (deprecated v 12+ konzumér CPU).
- **Návrh:** primárne TDX (home server target), SEV-SNP pre enterprise/cloud, SGX skip.

### OQ-08 (M2) Multi-tenant podpora
- Má jeden vault držať agentov pre viacerých adminov?
- Pridáva komplexitu RBAC + isolation; jednoduchšie je single-tenant.
- **Návrh:** single-tenant pre M1/M2; multi-tenant možno v M3 alebo enterprise edition.

### OQ-09 (M2) Smart account integration — vlastný validator vs Safe/SimpleAccount
- Vlastný ERC-4337 validator dáva flexibilitu (TEE attestation), ale audit cost.
- Safe modul je rýchlejší, ale obmedzuje attestation flow.
- **Návrh:** v M2 podpis cez existujúce Safe/SimpleAccount; v M3 vlastný validator.

### OQ-10 (M3) On-chain audit anchor — ktorý chain
- Base, Polygon zkEVM, Arbitrum sú lacné. Čisto historický anchor.
- Občas argumenty pre Bitcoin OP_RETURN (immutability), ale drahé.
- **Návrh:** konfigurovateľné, default Base.

### OQ-11 (M2) Push notification relay — vlastný hosting alebo open-source
- Vlastný relay = sovereignty + privacy, ale operational cost.
- ntfy.sh / Gotify ako self-hosted alternatíva.
- **Návrh:** podporovať self-hosted (ntfy default), nehostovať pre uživateľov.

### OQ-12 (M1) Recovery model
- Shamir secret sharing pre admin keys?
- Paper backup s passphrase?
- Hardware FIDO2 keys ako recovery?
- **Návrh:** dokumentovať tri patterny, používateľ si zvolí.

### OQ-13 (M2) Anomaly detection — pravidlový vs ML
- Pravidlový (frekvencia, geografia, suma odchýlka) je jednoduchý a auditovateľný.
- ML model prináša vyšší recall, ale je opaque (black box).
- **Návrh:** pravidlový baseline pre M2; ML iba ako optional komponent.

### OQ-14 (M3) Pricing / business model
- Open-source community edition + enterprise edition?
- Hosted version vs purely on-prem?
- Default policy library ako paid subscription (curated providers)?
- **Návrh:** open-source Apache 2.0 core + voliteľná Hosted Policy Library subscription pre enterprise.

### OQ-15 ✅ UPDATED (2026-04-27) Naming
- **Pôvodný stav:** "Agent Vault OS" implikovalo "operating system" — prehnané. "Local Agent Trezor" — kolízia s Trezor brandom. "Agent Payment Coprocessor" — dlhé.
- **Decision:** **SBO3L** je public brand / hackathon submission name.
- **Tagline:** "Spending mandates for autonomous agents."
- **Pitch veta:** "Don't give your agent a wallet. Give it a mandate."
- **Technical namespace:** `mandate` je finálny implementation namespace pre daemon, crates, schema IDs, CLI, cesty a interné dokumenty.
- **Reason:** `SBO3L` vytvára kategóriu: agent nemá wallet, má obmedzený a auditovateľný mandát. Jeden názov v pitchi aj v kóde znižuje kognitívny šum pre porotu aj developerov.
- **Open follow-up:** overiť trademark/domain/GitHub/ENS dostupnosť pre `SBO3L`, `Mandate402`, `mandate-agent`, `sbo3l.dev`, `mandate.eth`. Ak je `SBO3L` kolízne, fallback je **Mandate402**.

### OQ-16 (M2) Compliance / regulatory positioning
- Sme money transmitter? — nie, ak iba podpisujeme používateľove transakcie zo svojho HW.
- KYT (know-your-transaction) integration pre enterprise?
- GDPR — minimal PII, ale audit log môže obsahovať identifikátory.
- **Návrh:** legal review pred M2 release.

### OQ-17 (M3) Konfliktné architektonické varianty
- Niektorí users budú chcieť V3 (HSM only, no TEE) ako "production-good-enough".
- Iní budú chcieť V4 (full TEE).
- **Návrh:** explicitne komunikovať, že V3 je validná production konfigurácia; V4 je premium tier.

---

## R — Riziká

### R-01 — Adopcia x402
- **Risk:** x402 nemusí byť široko adoptovaný; agent ekonomika sa môže rozvíjať cez iné protokoly (l402, vlastné API key billing).
- **Mitigation:** modulárny payment_protocol layer; podpora viacerých protokolov.

### R-02 — Konkurencia z Coinbase / OpenAI / Anthropic
- **Risk:** veľký hráč spustí podobný produkt s lepším distribution channel.
- **Mitigation:** sovereignty pitch (lokálnosť, open-source, no vendor lock-in) — ťažko replikovateľný custodial vendormi.

### R-03 — TEE supply chain trust
- **Risk:** Intel/AMD/AWS root keys sú centralizovaný trust point.
- **Mitigation:** dokumentovať explicitne; podporiť multi-vendor attestation (defense-in-depth).

### R-04 — TEE side-channel zraniteľnosti
- **Risk:** historicky veľa CVE; nové sa objavujú.
- **Mitigation:** monitor security advisories; rapid update flow; HSM stále in the loop ako fallback.

### R-05 — Komplexnosť deploymentu
- **Risk:** bežný developer to nevie nainštalovať; adopcia stagnuje.
- **Mitigation:** docker compose quickstart (M1), .deb/.rpm (M2), appliance image (M3).

### R-06 — Prompt injection v reálnom svete
- **Risk:** policy nemôže pokryť všetky kreatívne prompt injection vektory.
- **Mitigation:** defense-in-depth — schema constraints + simulator + provider allowlist + budget caps + human approval; nikdy sa nespoliehať na jediný layer.

### R-07 — HSM vendor risk
- **Risk:** YubiHSM/Nitrokey môžu mať nepublikované firmware bugy alebo backdoory.
- **Mitigation:** dual-vendor support; preferovať open-source firmware (Nitrokey).

### R-08 — Operational dohľad
- **Risk:** používateľ nikdy nepozre audit log; freeze sa nestane keď treba.
- **Mitigation:** anomaly auto-freeze; mandatory weekly digest e-mail/notification.

### R-09 — Ekonomický model agentov je príliš nový
- **Risk:** agent ekonomika nie je connected reality; produkt je predčasný.
- **Mitigation:** pilot s reálnymi early adopters; iterácia na use cases (research agents, trading bots).

### R-10 — Právne riziká okolo "trezor" naming
- **Risk:** Trezor brand kolízia.
- **Mitigation:** vyhnúť sa "Trezor" v marketing; codename interný, brand later.

---

## Q — Otvorené technické experiments needed

1. **Latency benchmark:** koľko reálne trvá Rego eval + simulator + HSM sign na low-end NUC pre P99?
2. **PKCS#11 cross-vendor compatibility:** YubiHSM vs Nitrokey vs SoftHSM identický code path?
3. **TDX boot time:** ako dlho trvá TDX VM warm boot? Ovplyvní recovery flow?
4. **x402 corpus:** zber 50+ reálnych x402 challenges od rôznych providerov pre parser robustness.
5. **Reproducible build:** Rust cargo reproducibility na multi-arch; SBOM cez `cargo-cyclonedx`.
6. **Smart account validator gas cost:** ERC-4337 validator s TEE attestation verification — koľko gas?

---

## D — Decisions log (počiatočný)

| D# | Date | Decision | Rationale |
|---|---|---|---|
| D-001 | 2026-04-27 | Product + technical namespace: `SBO3L` / `mandate` | Jednotný brand aj implementation namespace |
| D-002 | 2026-04-27 | Public brand: **SBO3L** | Výraznejšie pre hackathon; agent nedostáva wallet, dostáva mandát |
| D-003 | 2026-04-25 | Primárny implementačný jazyk: Rust | Memory safety, single binary, dobre crypto |
| D-004 | 2026-04-25 | Primárna policy DSL: Rego (cez regorus) | Mature, expressive, embedded |
| D-005 | 2026-04-25 | Storage: SQLite (M1/M2) | Single-file, atomic, well-tested |
| D-006 | 2026-04-25 | Architektúra cieľová: V4 (TEE + HSM) | Najsilnejšia bezpečnostná postura |
| D-006 | 2026-04-25 | License: Apache 2.0 core | Maximalna adopcia |

(Decisions log bude pokračovať počas vývoja; každé reverzné rozhodnutie potrebuje zápis.)
