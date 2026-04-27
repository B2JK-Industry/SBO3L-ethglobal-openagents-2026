# F. Reference Architecture — 4 varianty

Štyri varianty s rastúcou bezpečnosťou a komplexnosťou. Variant 1 je MVP, Variant 4 je cieľová unikátna architektúra.

---

## Variant 1 — MVP Soft Isolation

**Komponenty:**
- `mandate` Linux service (systemd unit) — single binary (Rust alebo Go).
- Encrypted key file (age/sops/libsodium-secretstream), passphrase v env var alebo cez systemd credentials.
- Policy engine in-process (Rego embedded cez `regorus` alebo CEL evaluator).
- Local REST + gRPC + Unix socket API.
- SQLite audit log s hash chain (tabuľka `events` s `prev_hash`).
- CLI (`mandate admin policy ...`, `mandate admin budget ...`).
- Web UI (lokálny, lokálne loopback iba).

**Data flow:** agent → Unix socket → vault process (in-process: validator → policy → ledger → simulator → signer in-memory) → audit → response.

**Security properties:**
- Key encrypted at rest.
- Policy engine v separátnom logical scope (ale rovnaký proces).
- Audit hash-chained.
- Žiadny network listening externe (iba lokálne).

**Limitations:**
- Root vidí pamäť procesu → môže exfiltrovať key počas behu.
- Single process compromise = total compromise.
- Žiadna attestation.
- Compromised host OS = total compromise.

**Na čo je dobrý:** PoC, dev environment, low-stakes pilot u early adoptera. Validácia API, policy DSL, UX flow.

**Prečo NIE je produkčne dostatočný:** Threat #5 (compromised host OS) a #17 (key exfiltration) nie sú adekvátne mitigované.

**Čo overí:** že policy DSL funguje, že x402 verifier rieši reálne providerov, že agent integration je použiteľná.

**MVP suitability:** 10/10. **Production:** 3/10. **Complexity:** 1/5.

---

## Variant 2 — VM Isolation

**Komponenty:**
- **Agent VM** (KVM/QEMU/Firecracker/microVM) — beží AI agent + tools.
- **Vault VM** — beží `mandate` daemon.
- **Network bridge:** vsock (KVM) alebo Unix socket forwarding cez virtio-vsock; *žiadny IP routing medzi VMs okrem explicitného portu*.
- Encrypted key vo Vault VM (LUKS-on-LVM + key file unsealed cez TPM-bind).
- Audit log vo Vault VM.

**Data flow:** agent (Agent VM) → vsock → vault process (Vault VM) → in-process pipeline → audit → response.

**Security properties:**
- Kompromitácia Agent VM neprenikne automaticky do Vault VM (KVM hypervisor barrier).
- TPM-bound key sa neodomkne na inom HW (ukradnutý disk → nepoužiteľný).
- Možnosť oddelenej network policy (Vault VM má iba egress k whitelistovaným RPC providerom).

**Výhody:**
- Hypervisor je menší attack surface než celý userspace.
- Možnosť snapshot/rollback Vault VM.
- Jasná operational separation.

**Nevýhody:**
- Komplexnejšia inštalácia pre koncového dev usera.
- Hypervisor escape (vzácny ale existuje) = kompromitácia.
- Stále nemá *attestation* — Vault VM nevie dokázať, že beží správny image.

**Vhodnosť pre lokálny server:** dobrá — väčšina mini-PC/NUC podporuje KVM bez problémov.

**MVP suitability:** 5/10 (overhead). **Production:** 6/10. **Complexity:** 3/5.

---

## Variant 3 — TPM / HSM-backed Signing

**Komponenty:**
- Policy engine v Linuxe (alebo vo Vault VM ako vo V2).
- **Signing key v TPM 2.0 alebo HSM** (YubiHSM 2, Nitrokey HSM 2, alebo SoftHSM pre dev).
- **PKCS#11 interface** medzi vaultom a HSM — signer je nahradený PKCS#11 client wrapperom.
- HSM má per-key constraints: sign only, no extraction, optional usage counter.
- Audit log lokálne + HSM internal log (ak vendor podporuje).
- Human policy UI cez admin CLI s admin signing key tiež v HSM (separátny slot).

**Čo rieši HSM:**
- Threat #17 (key exfiltration) — key opúšťa HSM iba ako signature.
- Threat #5/6 (root compromise key disclosure) — root nedostane plaintext key.
- Kryptograficky atestovaný origin podpisu (ak HSM podporuje attestation).

**Čo HSM NErieši:**
- *Aký* payload sa podpisuje. HSM je hlúpy — podpíše čo dostane, ak má key handle.
- Preto **policy musí byť stále vyriešená pred HSM volaním**.
- HSM nevie, že "0x..." je USDC transfer na nesprávny contract. To musí policy engine + simulator.

**Kde musí byť policy:** v procese pred HSM volaním, ideálne v izolovanom procese alebo VM (V2 + V3 kombo). HSM je iba *enforcement vrstva pre integritu kľúča*, nie pre integritu payloadov.

**Limitations:**
- Policy engine stále zraniteľný voči host OS compromise.
- Bez attestation runtime nikto neoverí, že to bol skutočne náš policy engine, ktorý poslal podpisovú žiadosť.

**MVP suitability:** 4/10 (HSM nákup + integrácia). **Production:** 7/10. **Complexity:** 3/5.

---

## Variant 4 — TEE Policy Wallet + HSM Signer (cieľová unikátna architektúra)

**Komponenty:**
- **Agent runtime** v normálnom Linuxe (nie v TEE).
- **Policy Engine v TEE** — Intel TDX VM alebo AMD SEV-SNP confidential VM (cieľ); fallback Intel SGX enclave (Gramine/Occlum) pre špecifické komponenty.
- **Sealed policy storage:** policy konfigurácia podpísaná adminom + sealed cez TEE-specific sealing (TPM-derived key bound na PCR + SGX-sealed alebo TDX/SEV measurement-derived).
- **Key v HSM alebo TEE-sealed storage** (preferovane HSM pre defense-in-depth).
- **Attestation:** policy engine generuje attestation evidence (Intel DCAP quote, AMD SEV-SNP attestation report, alebo Nitro attestation document v cloud variante) pri každom decision (alebo periodicky).
- **Signed policy decisions:** policy engine podpíše decision attestation-bound kľúčom; HSM odmietne podpis bez tohto tokenu.
- **x402 verifier** v TEE — nedôverujeme provider response pred verifikáciou.
- **Audit evidence:** každý audit záznam má attestation reference (measurement hash + decision signature).

**Komponenty toku:**

```
Agent runtime (untrusted)
   │
   ▼ vsock/Unix socket
Gateway (controlled, hardenovaný proxy)
   │
   ▼ vsock / TEE-attested channel
┌──────────── TEE Policy Engine (TDX/SEV-SNP) ────────────┐
│  schema validator → policy eval → ledger → simulator   │
│  → x402 verifier → decision signer (attestation-bound) │
└────────────────────┬───────────────────────────────────┘
                     │ signed decision + tx template
                     ▼
              HSM signer (verifies decision sig + attestation token)
                     │
                     ▼ signature
              Audit log writer (TEE) ──► append-only signed log
                     │
                     ▼ Merkle root daily
              On-chain anchor (optional)
```

**Security properties:**
- Compromised host OS *nedokáže* exfiltrovať key (HSM) ani odpočúvať policy decisions (TEE pamäťovo izolovaná).
- Compromised host OS *nedokáže* tichym podstrčením zmenenej policy spôsobiť zlý podpis — TEE attestation by sa zmenila a HSM by tx odmietla podpísať.
- Externý smart account vie *on-chain* verifikovať TEE attestation (cez verifikované TEE root certs, napr. Intel DCAP) → ERC-4337 validator akceptuje iba podpisy s platnou attestation.
- Audit log je signed v TEE → tampering po-fakte detekovateľné.

**Limitations:**
- TDX/SEV-SNP HW dostupnosť na home setupe (rastie — Intel 4th-gen Xeon, AMD EPYC Genoa/Bergamo; konzumér: Intel 12th gen+ má TDX čiastočne).
- Komplexnosť deploymentu.
- TEE side-channel história (Spectre, Foreshadow, ...) — mitigated v moderných verziách, ale *zbytkové* riziko existuje.
- Attestation supply chain (Intel/AMD root keys) je centralizovaný trust point.

**Estimated complexity:** 5/5. **Production suitability:** 9/10. **MVP suitability:** 1/10 (príliš veľa pre prvý release).

**Dependencies:** TEE-capable hardware, HSM vendor SDK, attestation verifier service, smart account integration (pre on-chain verification).

---

## F.5 Súhrnné porovnanie

| Vlastnosť | V1 Soft | V2 VM | V3 HSM | V4 TEE+HSM |
|---|---|---|---|---|
| Key isolation | encrypted file | encrypted file + TPM bind | **HW (HSM)** | **HW (HSM)** |
| Policy isolation | žiadna | VM | VM | **TEE** |
| Attestation | žiadna | žiadna | čiastočne (HSM key origin) | **plné runtime + policy attestation** |
| Compromised host OS surveys | ❌ | ⚠ čiastočne | ✅ key | ✅ key + policy |
| Compromised root | ❌ | ❌ | ✅ key | ✅ key + policy |
| Smart account on-chain verification | ❌ | ❌ | ❌ | **✅** |
| HW dostupnosť | každý linux | KVM-capable CPU | + HSM ($150–500) | + TEE CPU + HSM |
| MVP fit | ✅ | ⚠ | ❌ | ❌ |
| Production cieľ | ❌ | ⚠ stredné riziko | ✅ | **✅✅** |
| Komplexita 1–5 | 1 | 3 | 3 | 5 |

**Stratégia:** V1 ako MVP (mesiac 0–3), V3 ako "production HSM tier" (mesiac 4–9), V4 ako "enterprise/agent-economy tier" (mesiac 9–18).
