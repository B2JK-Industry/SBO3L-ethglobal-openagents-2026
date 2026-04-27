# D. Threat Model

## D.1 Aktéri a aktíva

**Aktéri (potenciálni útočníci):**
- *External attacker* — internet, kompromitovaný x402 provider, MITM.
- *Compromised agent process* — RCE, prompt injection, malicious tool.
- *Compromised host OS* — root user, malware, kernel exploit.
- *Insider* — admin so zlými úmyslami alebo s ukradnutým prístupom.
- *Supply chain attacker* — kompromitovaná dependency, kompromitovaný update.
- *Physical attacker* — ukradnutý disk/notebook.

**Aktíva (čo chránime):**
- Private key (najvyššia hodnota).
- Treasury / on-chain balance.
- Policy konfigurácia (jej integrita).
- Audit log (jeho integrita).
- Attestation kľúče.
- Operational continuity (anti-DoS).

**Trust assumptions (čomu *musíme* veriť):**
- TEE / HSM / TPM hardvér nie je backdoorovaný.
- Crypto knižnice (signal, secp256k1) sú korektné.
- Človek-admin v pokojnom stave nekoná zlovôľne.
- Recovery procedure nie je kompromitovaná v čase setupu.

---

## D.2 Tabuľka 25 útokov

| # | Útok | Pravdep. | Dopad | Mitigácia (MVP) | Mitigácia (production) | MVP stačí? |
|---|---|---|---|---|---|---|
| 1 | **Agent prompt injection** (užívateľský prompt prinúti agenta poslať zlú payment request) | Vysoká | Vysoký | Schema validácia + policy engine odmietne neznámeho recipienta + per-tx limit + transaction simulator | + human approval nad threshold + provider reputation score + anomaly detection nad spending patternom | Áno (s tightenými policy) |
| 2 | **Agent tool injection** (kompromitovaný tool/plugin pošle žiadosť) | Vysoká | Vysoký | Tool-level allowlist v policy + per-tool budget | + tool signing/manifest + capability tokens | Áno |
| 3 | **Compromised plugin/tool** (legit plugin s malicious update) | Stredná | Vysoký | Locked dependency hashes + isolated tool execution | + reproducible builds + sigstore + plugin allowlist v policy | Čiastočne |
| 4 | **Compromised agent process** (RCE) | Stredná | Stredný (key nie je v procese) | Key isolation cez encrypted file + policy engine v inom procese | + TEE policy engine + HSM signer + namespace/seccomp izolácia | Áno |
| 5 | **Compromised host OS** (kernel exploit, malware ako root) | Nízka | **Kritický** | Root vidí encrypted key file (ale nie unsealed); policy konfigurácia podpísaná | TEE-sealed key (root nedostane plaintext) + remote attestation + sealed policy storage | **Nie** — production nutne TEE/HSM |
| 6 | **Compromised root user** | Nízka | Kritický | Audit log podpísaný (root vidí, ale tampering odhalí externý verifier) | TEE prevention + HSM odmietne podpis bez attestation tokenu | Nie |
| 7 | **Ukradnutý disk** | Stredná | Vysoký | LUKS full-disk encryption + key file šifrovaný passphrase / TPM sealed | TPM-sealed key (key sa neodomkne na inom HW) | Áno |
| 8 | **Ukradnutý notebook** | Stredná | Vysoký | Auto-lock + LUKS + recovery procedure (revoke on-chain) | + remote kill switch + smart account on-chain freeze | Áno |
| 9 | **Malware na Linuxe (non-root)** | Stredná | Stredný | User-level izolácia (vault beží pod separátnym UID, socket s 0600 perms) | + AppArmor/SELinux profile + seccomp + namespace | Áno |
| 10 | **Supply chain útok na vault binary** | Nízka | Kritický | Pinned dependencies + SHA verification | Reproducible builds + sigstore + transparency log + binary attestation | Nie |
| 11 | **Falošný x402 provider** (fishing endpoint) | Vysoká | Stredný | Provider allowlist + domain pinning + cert pinning | + provider reputation oracle + on-chain registry | Áno |
| 12 | **DNS hijacking** | Stredná | Stredný | TLS cert pinning + DoH/DoT | + DNSSEC + IP-level allowlist | Áno |
| 13 | **MITM** | Nízka (s TLS) | Vysoký | mTLS pre upstream, cert pinning pre x402 providerov | + post-quantum hybrid TLS option | Áno |
| 14 | **Replay attack payment requestu** | Stredná | Stredný | nonce + expiry v request schéme + idempotency key | + nonce v signed payload + on-chain replay protection (smart account) | Áno |
| 15 | **Agent obíde policy engine** (priamo zavolá signer) | Stredná | Kritický | Signer prijíma iba payloady podpísané policy enginom (internal HMAC/signature) | + TEE attestation token v každej signing request + capability-based architecture | Áno |
| 16 | **Agent zmení allowlist** | Stredná | Vysoký | Policy storage iba writable adminom; agent nemá filesystem write k policy/ | + signed policy + change requires admin signature + audit log na každej zmene | Áno |
| 17 | **Exfiltrácia private key** | Stredná | Kritický | Encrypted file s passphrase mimo procesu (env var / prompt) | TPM/HSM/TEE — key nie je nikdy v plaintexte v RAM hosta | **Nie** — production HSM/TEE |
| 18 | **HSM použitý na podpis zlej transakcie** (HSM podpíše čo dostane) | Vysoká pri zlej arch | Kritický | Policy engine vždy pred HSM volaním + signer accepts only normalized & policy-signed payloads | + per-key constraints v HSM (PKCS#11 attribútov) + on-chain validator (smart account) | Áno |
| 19 | **Insider attack** (admin) | Nízka–stredná | Kritický | Audit log + 2-osobová zmena policy nad threshold | M-of-N admin schválenia + hardware-backed admin keys + delayed execution okno na revert | Čiastočne |
| 20 | **Zle nastavený limit** (admin omyl) | Vysoká | Stredný | Defaults sú konzervatívne + dry-run režim + policy linter | + policy simulation/replay nad historickými requestami + canary phase | Áno |
| 21 | **Nekonečná slučka mikroplatieb** | Stredná | Vysoký | Rate limiting + per-task budget + per-minute cap | + automatic circuit breaker + anomaly detection | Áno |
| 22 | **Forked/malicious chain RPC** | Stredná | Vysoký | Multi-RPC quorum + chain ID pinning | + Merkle proof verification + light client | Áno |
| 23 | **Transaction simulation mismatch** (simulation OK, real fail/different) | Stredná | Stredný | Simulator volá rovnaký RPC ako broadcast + state-pinned simulation | + commit-reveal pattern + same-block submission | Áno |
| 24 | **Upgrade attack na policy engine** (rogue update zmení správanie) | Nízka | Kritický | Signed releases + manuálny `mandate upgrade` s hash checkom | + reproducible builds + transparency log + delayed activation + rollback window | Čiastočne |
| 25 | **Log tampering** | Stredná | Vysoký | SQLite + hash chain + lokálne podpísané záznamy | + Merkle root publikovaný on-chain alebo do externého storage (S3 object lock, IPFS pinned) | Áno |

---

## D.3 STRIDE per komponente (zhrnutie)

| Komponent | Spoofing | Tampering | Repudiation | Info disclosure | DoS | Elevation |
|---|---|---|---|---|---|---|
| Agent Runtime | Vysoké | Vysoké | Vysoké | Stredné | Stredné | Vysoké |
| Policy Engine | Stredné | **Kritické** | Stredné | Stredné | Stredné | Vysoké |
| Signing Vault | Nízke (ak attestation) | **Kritické** | Nízke (ak audit) | **Kritické** (key) | Stredné | **Kritické** |
| Audit Log | Stredné | **Vysoké** | Vysoké (ak chýba podpis) | Nízke | Nízke | Nízke |
| Human UI | Stredné | Stredné | Nízke | Stredné | Nízke | Vysoké (admin) |

---

## D.4 Pravidlá obhajoby (defense-in-depth invariant)

1. **Žiadne rozhodnutie bez policy.** Ani admin nemôže obísť policy engine, len ho zmeniť (a aj to s auditom).
2. **Žiadny podpis bez rozhodnutia.** Signer nikdy nepodpíše payload bez interného dôkazu, že prešiel policy enginom (HMAC/signature, idealne attestation token).
3. **Žiadna zmena policy bez auditu.** Každá policy mutation = nová verzia + hash + admin podpis + audit event.
4. **Žiadny audit log bez integrity.** Hash chain + denný external Merkle root + (production) on-chain anchor.
5. **Žiadny key v RAM agenta.** Nikdy. Ani počas debug. Ani počas testov. Test vault má testovací key, nie produkčný.
6. **Žiadna automatická upgrade kritickej cesty.** Policy engine a signer = manual upgrade s hash verifikáciou.
