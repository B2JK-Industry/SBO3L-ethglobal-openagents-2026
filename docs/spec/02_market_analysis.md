# B. Market & Existing Solutions Analysis

## B.1 Mapovanie existujúcich riešení

### Klasické hardvérové peňaženky (Trezor, Ledger)
- **Čo riešia:** offline úschova kľúča, manuálne potvrdenie každej transakcie tlačidlom.
- **Čo neriešia:** programovateľné policy, automatizované mikroplatby, agentický flow, machine-to-machine.
- **Vhodné pre AI agentov:** nie. Každá transakcia = fyzický klik.
- **Agent vidí kľúč:** nie.
- **Policy layer:** žiadny (len podpis/odmietnutie).
- **Attestation:** áno (firmware), ale neviazaná na policy.
- **Lokálne na vlastnom Linuxe:** áno (USB), ale UX brzdí autonomiu.
- **Medzera:** chýba policy engine, budget ledger, x402, audit log, programovateľnosť.

### Server walety (geth keystore, ethers.js Wallet, web3.js, Foundry cast wallet)
- **Čo riešia:** podpisovanie z procesu, automatizácia.
- **Čo neriešia:** izoláciu kľúča od agent runtime, policy, audit, attestation.
- **Vhodné pre AI agentov:** povrchne áno, bezpečnostne nie.
- **Agent vidí kľúč:** áno (alebo má prístup k unsealed file).
- **Policy layer:** žiadny.
- **Attestation:** žiadna.
- **Lokálne:** áno.
- **Medzera:** všetko okrem podpisu.

### Embedded wallets (Privy, Magic, Web3Auth, Dynamic)
- **Čo riešia:** UX onboarding pre koncových userov, social login.
- **Čo neriešia:** autonómny agent payment flow, lokálnu kontrolu, policy nad pravidlami platieb.
- **Vhodné pre AI agentov:** nie, sú user-centric.
- **Agent vidí kľúč:** závisí (typicky cez SDK volanie do hosted infra).
- **Policy layer:** minimálny (auth-level).
- **Attestation:** typicky nie.
- **Lokálne:** nie (cloud-hosted).
- **Medzera:** úplne iný use case.

### MPC wallets (Fireblocks, Fordefi, Copper, Cobo, Safeheron)
- **Čo riešia:** distribuovaný podpis, threshold signatures, enterprise treasury.
- **Čo neriešia:** lokálne, agent-friendly mikroplatby s low latency; väčšinou custodial.
- **Vhodné pre AI agentov:** čiastočne (Fireblocks má policy engine), ale cloud-bound.
- **Agent vidí kľúč:** nie.
- **Policy layer:** áno.
- **Attestation:** SOC2 audit, nie cryptographic per-tx.
- **Lokálne:** nie.
- **Medzera:** lokálnosť, sovereignty, otvorenosť, latencia mikroplatieb, cena.

### Turnkey
- **Čo rieši:** managed key infra ako API, policy quorum, raw transaction signing.
- **Čo nerieši:** lokálnosť, sovereignty, x402 native, attestation k policy hashu.
- **Vhodné pre AI agentov:** áno, použiteľné, ale custodial.
- **Agent vidí kľúč:** nie.
- **Policy layer:** áno (basic).
- **Attestation:** nie (cloud-trusted).
- **Lokálne:** nie.
- **Medzera:** dôvera v Turnkey ako voči vlastnému HW.

### Coinbase Agentic Wallets / CDP Wallets / AgentKit
- **Čo rieši:** agent-friendly wallets ako služba, integrácia s Base, x402 native.
- **Čo nerieši:** lokálnu sovereignty, vlastný HW, on-prem deployment, vlastnú policy infra mimo Coinbase.
- **Vhodné pre AI agentov:** explicitne navrhnuté pre agentov.
- **Agent vidí kľúč:** nie (CDP-managed).
- **Policy layer:** áno (CDP).
- **Attestation:** nie (cloud trust).
- **Lokálne:** nie.
- **Medzera:** vendor lock-in, vlastná infraštruktúra, regulatorná dôvera.

### Safe (Gnosis Safe) smart accounts
- **Čo rieši:** multisig, on-chain policy, modules, guards, recovery.
- **Čo nerieši:** off-chain agent decision flow, x402 latency, lokálny key management (stále potrebujete signera).
- **Vhodné pre AI agentov:** áno ako settlement layer.
- **Agent vidí kľúč:** nie (záleží na signeroch).
- **Policy layer:** áno (on-chain).
- **Attestation:** on-chain proofs, nie attestation runtime.
- **Lokálne:** áno z hľadiska deploymentu kontraktu.
- **Medzera:** plný flow off-chain neexistuje, gas overhead pri mikroplatbách.

### ERC-4337 account abstraction + session keys
- **Čo rieši:** session keys s obmedzeniami, paymaster, bundler, modular validation.
- **Čo nerieši:** off-chain audit, attestation runtime, x402 protokol, hardvérovú izoláciu session key.
- **Vhodné pre AI agentov:** áno, navrhnuté pre delegated/limited access.
- **Agent vidí kľúč:** typicky áno (session key v jeho procese).
- **Policy layer:** áno (on-chain validator).
- **Attestation:** nie.
- **Lokálne:** áno (off-chain session key).
- **Medzera:** session key musí žiť niekde — mandate je to "niekde".

### YubiHSM 2, Nitrokey HSM 2, smart cards (PKCS#11)
- **Čo rieši:** hardvérová izolácia kľúča, signing primitives, audit eventov.
- **Čo nerieši:** policy semantics (blockchain-aware), x402, transaction simulation.
- **Vhodné pre AI agentov:** ako stavebný blok, nie ako produkt.
- **Agent vidí kľúč:** nie.
- **Policy layer:** veľmi obmedzený (key usage flags).
- **Attestation:** áno (vendor attestation).
- **Lokálne:** áno (USB/PCIe).
- **Medzera:** HSM nevie, čo je to "x402 challenge" alebo "USDC transfer na neoverený contract".

### HashiCorp Vault Transit
- **Čo rieši:** central key management, encryption-as-a-service, basic policy.
- **Čo nerieši:** chain-aware semantics, x402, mikroplatby, attestation.
- **Vhodné pre AI agentov:** ako auxiliary, nie primary.
- **Agent vidí kľúč:** nie.
- **Policy layer:** áno (HCL).
- **Attestation:** nie.
- **Lokálne:** áno.
- **Medzera:** general-purpose, bez doménovej znalosti.

### AWS KMS / CloudHSM / Nitro Enclaves
- **Čo rieši:** managed HSM, attested confidential compute (Nitro).
- **Čo nerieši:** lokálny on-prem deployment, sovereignty.
- **Vhodné pre AI agentov:** áno (Nitro Enclaves sú ideálny TEE pattern).
- **Agent vidí kľúč:** nie.
- **Policy layer:** áno (KMS policies).
- **Attestation:** áno (Nitro Attestation Document).
- **Lokálne:** nie.
- **Medzera:** AWS-bound. Patternový vzor pre náš produkt, nie konkurent.

### TEE-based wallet infra (Fordefi MPC, Phala, Marlin, Oasis, Secret)
- **Čo rieši:** confidential compute pre wallet operácie alebo confidential transactions.
- **Čo nerieši:** lokálny home server use case, agent-specific x402 flow.
- **Agent vidí kľúč:** nie.
- **Policy layer:** závisí (Phala má, Oasis má).
- **Attestation:** áno.
- **Lokálne:** väčšinou nie.
- **Medzera:** chýba lokálna distribuovateľná open-source verzia ladená na agent payments.

### MPC custody (Fireblocks Network, BitGo, Anchorage)
- **Čo rieši:** enterprise custody, regulácia, insurance.
- **Čo nerieši:** lokálnosť, agent-native flow.
- **Medzera:** opačný koniec spektra, nie konkurent na home server.

### Smart contract walety (Argent, Ambire, Coinbase Smart Wallet)
- **Čo rieši:** social recovery, gas sponzorstvo, pluginy.
- **Čo nerieši:** off-chain agent decision a key isolation.
- **Medzera:** komplementárne — mandate môže byť signer pre takýto kontrakt.

### x402 payment infrastructure (Coinbase x402, l402 Lightning)
- **Čo rieši:** standard pre HTTP 402 platby, machine-to-machine UX.
- **Čo nerieši:** kde žije kľúč, kto rozhoduje, ako sa to auditovuje.
- **Medzera:** protokol, nie execution environment. Náš produkt je *peer* k x402 — implementuje x402 verifier ako prvotriedneho občana.

---

## B.2 Porovnávacia tabuľka

| Riešenie | Key isolation | Policy engine | Agent-ready | Local Linux | Attestation | Hlavná slabina |
|---|---|---|---|---|---|---|
| Trezor / Ledger | HW (silná) | žiadna | nie | čiastočne | firmware | manuálny klik na každú tx |
| geth / ethers signer | žiadna | žiadna | technicky áno | áno | nie | key v RAM agenta |
| Privy / Magic | cloud | minimálna | nie | nie | nie | user-centric, custodial |
| Fireblocks | MPC | áno | čiastočne | nie | enterprise audit | cloud, drahé, slow |
| Turnkey | TEE (cloud) | áno | áno | nie | čiastočne | custodial, vendor trust |
| Coinbase Agentic / CDP | cloud | áno | natívne | nie | nie | vendor lock-in |
| Safe smart account | závisí od signera | on-chain | áno | áno | on-chain proof | vyžaduje signer infra |
| ERC-4337 session keys | žiadna (off-chain key) | on-chain validator | áno | áno | nie | session key musí niekde žiť |
| YubiHSM / Nitrokey HSM | HW | minimálna | čiastočne | áno | vendor | nevie chain semantics |
| HashiCorp Vault Transit | SW + HSM backend | áno (HCL) | čiastočne | áno | nie | nie chain-aware |
| AWS KMS / Nitro | HW + TEE | áno | áno | nie | áno | AWS lock-in |
| Phala / Oasis confidential | TEE | áno | čiastočne | nie | áno | nie pre home server |
| **mandate (cieľ)** | **HW/TEE/HSM** | **áno (Rego/CEL, signed)** | **natívne** | **áno** | **áno (TEE)** | **nový produkt, nutná adopcia** |

---

## B.3 Mapa medzery

Celá súčasná ponuka spadá do jedného z troch klastrov:

1. **Custodial agent wallets** (Coinbase, Turnkey, Privy) — pohodlné, ale vendor-bound a obetujú sovereignty.
2. **Hardvérové trezory pre ľudí** (Trezor, Ledger) — bezpečné, ale UX neumožňuje autonómiu.
3. **Enterprise key management** (HSM, Vault, KMS) — robustné, ale nevedia o blockchaine, x402, ani o agentoch.

**Medzera, ktorú zapĺňa sbo3l:**
> Lokálny, otvorený, agent-natívny, policy-driven, hardvérovo-izolovaný a attestable platobný koprocesor pre vlastný Linux server.

Žiadne existujúce riešenie nepokrýva všetky štyri vlastnosti naraz: **lokálnosť + agentic-natívnosť + policy-as-code + attestable hardware-isolated signing**.
