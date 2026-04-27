# E. Trust Boundary Architecture

## E.1 Šesť zón

```
┌──────────────────────────────────────────────────────────────────────┐
│  6. AUDIT ZONE                  (append-only, signed, exportable)    │
│     ↑ jednosmerný zápis                                              │
├──────────────────────────────────────────────────────────────────────┤
│  5. HUMAN GOVERNANCE ZONE       (admin UI/CLI, recovery, multisig)   │
│     ↓ podpísané policy mutations / approvals                         │
├──────────────────────────────────────────────────────────────────────┤
│  4. SIGNING ZONE                (HSM/TPM/SE/TEE — kľúč žije iba tu)  │
│     ↑ accept iba authorized payloady                                 │
├──────────────────────────────────────────────────────────────────────┤
│  3. TRUSTED POLICY ZONE         (policy engine, ledger, risk)        │
│     ↑ signed decision + audit event                                  │
├──────────────────────────────────────────────────────────────────────┤
│  2. CONTROLLED ZONE             (gateway, normalizer, validator)     │
│     ↑ schema-validated request                                       │
├──────────────────────────────────────────────────────────────────────┤
│  1. UNTRUSTED ZONE              (agent runtime, LLM, tools, internet)│
└──────────────────────────────────────────────────────────────────────┘
```

---

## E.2 Detail zón

### Zone 1 — Untrusted Zone
**Obsah:** AI agent / LLM orchestrator, tools, MCP servers, plugin code, internet, externé API, x402 providers.
**Trust:** **0**. Predpokladáme, že hocikto z tejto zóny môže byť kompromitovaný (prompt injection, malicious tool, fishing provider).
**Prístup:** Iba cez Controlled Zone API (REST/gRPC/Unix socket). Žiadny priamy prístup k policy konfigurácii, k audit logu, ku kľúču, ani k signer procesu.
**Privilegia:** Môže iba *žiadať* (POST payment-request).

### Zone 2 — Controlled Zone (Gateway)
**Obsah:** Local API gateway, request normalizer, JSON schema validator, x402 challenge parser, rate limiter, authn/authz pre agent identity.
**Trust:** **1**. Kód, ktorému veríme do takej miery, že robí parsing nebezpečného vstupu — preto hardenovaný (memory-safe jazyk, fuzz tested, malý attack surface).
**Funkcia:** Premieňa surovú agent request na *normalizovaný internal payload*, ktorý je ďalej spracovateľný.
**Čo SMIE prejsť dovnútra (do Zone 3):** schémou validovaný request s priradeným agent identity tokenom.
**Čo NESMIE prejsť von (do Zone 1):** žiadne policy interné info okrem decision result a request ID.

### Zone 3 — Trusted Policy Zone
**Obsah:** Policy engine (Rego/CEL evaluator), policy database (versionovaná), budget ledger (SQLite/Postgres/embedded KV), risk scoring, approval workflow orchestrator, x402 verifier, transaction simulator client.
**Trust:** **2**. Najcitlivejší kód okrem signera.
**Funkcia:** Vyhodnocuje policy nad request + state, emituje *signed decision* (allow/deny/escalate).
**Cieľová verzia:** beží v TEE (SGX/TDX/SEV-SNP), s attestation evidence.
**Čo SMIE prejsť do Signing Zone:** signed decision payload (HMAC alebo Ed25519) obsahujúci normalizovaný transaction template.
**Čo NESMIE:** žiadny priamy prístup k unsealed key. Signer odmietne payload bez korektného decision podpisu.

### Zone 4 — Signing Zone
**Obsah:** HSM (YubiHSM/Nitrokey/CloudHSM), TPM 2.0, secure element, smartcard, TEE-sealed key storage. PKCS#11 interface alebo natívny SDK.
**Trust:** **3** (najvyšší). Hardware root of trust.
**Funkcia:** Prijíma signed decision payload + transaction template, overí decision signature, podpíše transakciu, vráti signature.
**Invariant:** Private key opúšťa Signing Zone iba vo forme cryptographic signature. Nikdy ako material.
**Čo SMIE prejsť von:** signature, public key, attestation.
**Čo NESMIE:** plaintext private key, seed, mnemonic.

### Zone 5 — Human Governance Zone
**Obsah:** Admin UI (web/desktop), CLI (`mandate admin ...`), recovery key holders, multisig participants, emergency stop button (HW alebo SW).
**Trust:** **2.5** (podmienečne; admin môže byť kompromitovaný preto multisig nad threshold).
**Funkcia:** Vytvára/mení policy, schvaľuje escalated requests, vykonáva emergency akcie.
**Boundary k Trusted Policy Zone:** Každá mutation je *podpísaná* admin kľúčom. Threshold mutation vyžaduje M-of-N podpisov. Audit log zaznamená každú mutation.
**Recovery:** Recovery kľúče sú offline (paper backup, Shamir secret sharing). Recovery procedure je *posledná inštancia* a vyžaduje multisig + delay window.

### Zone 6 — Audit Zone
**Obsah:** Append-only log (lokálny SQLite + hash chain), exporter (JSON/CSV), signed report generator, optional on-chain anchor.
**Trust:** **2** (write-only z policy a signing zón; read pre admin a externý auditor).
**Funkcia:** Zachytáva každý request, decision, signature, mutation, error, emergency event.
**Invariant:** Žiadny zápis sa nedá zmazať ani modifikovať bez detekcie (hash chain). Externý verifier vie nezávisle prejsť log a overiť integritu.
**Boundary:** Read-only pre admin/auditor. Write iba cez interné API z Zone 3 a 4.

---

## E.3 Pravidlá toku medzi zónami

| Z | → | Do | Smie prejsť | Nesmie prejsť |
|---|---|---|---|---|
| 1 Untrusted | → | 2 Controlled | Surová payment request (JSON), agent JWT/mTLS cert | Akýkoľvek control príkaz, raw transaction bez intent |
| 2 Controlled | → | 3 Policy | Normalizovaný request + agent identity | Surový vstup, untrusted strings v policy contexte |
| 3 Policy | → | 4 Signing | Signed decision + transaction template + nonce | Plaintext request bez signature, decision bez payload integrity |
| 4 Signing | → | 3 Policy | Signature, public key, error code | Plaintext key, seed |
| 4 Signing | → | 6 Audit | Signature event metadata (hash, ts, key id) | Plaintext payload (citlivé info) — iba hash |
| 3 Policy | → | 6 Audit | Decision event, full request hash, policy version | — |
| 5 Human | → | 3 Policy | Signed mutation, signed approval | Unsigned mutation (vždy odmietnutá) |
| 5 Human | → | 4 Signing | Signed emergency command (freeze, rotate) | Direct signing request bez policy passu |
| 6 Audit | → | 5 Human | Read-only export, signed report | Write access |
| 1 Untrusted | → | 4, 5, 6 | **NIKDY priamo** | Všetko |

---

## E.4 Capability model (alternatívne čítanie)

Každá zóna drží *capability* (autorizačný token), ktorý umožňuje konkrétnu akciu:

- `cap:request:create` — Zone 1 cez Zone 2 do Zone 3.
- `cap:decision:sign` — interný kľúč Zone 3, ktorým podpisuje decision pre Zone 4.
- `cap:transaction:sign` — drží ho Zone 4 (HSM key handle).
- `cap:policy:mutate` — Zone 5 admin podpis (M-of-N).
- `cap:emergency:freeze` — Zone 5 special key (často offline).
- `cap:audit:read` — Zone 5 reader role.
- `cap:audit:write` — interný; Zone 3 a 4 majú write-only handle.

Žiadna kapability nie je transferovateľná medzi zónami.
