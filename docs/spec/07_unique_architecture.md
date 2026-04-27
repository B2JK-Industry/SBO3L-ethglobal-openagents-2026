# G. Unique Architecture Proposal — 10 stavebných blokov

Toto je *unikátna* architektúra mandate. Každý z 10 blokov je špecifikovaný tak, aby fungoval ako samostatný modul s jasným kontraktom a aby ho bolo možné nezávisle vyvíjať, testovať a vymeniť.

---

## G.1 Agent Payment Request Protocol (APRP)

**Účel:** Definuje *jediný legitímny* spôsob, akým agent komunikuje s vaultom. Žiadne iné vstupy nie sú prijateľné.

**Kľúčový princíp:** Agent **NIKDY** neposiela raw transakciu. Posiela *intent* (čo chce dosiahnuť) a *payment context* (prečo, koľko, komu, za čo). Vault si transakciu skonštruuje sám.

**Schema (JSON):**

```jsonc
{
  "agent_id": "research-agent-01",                  // identita agenta (mTLS cert subject alebo JWT sub)
  "task_id": "task-2026-04-25-7f3a",                // logical work unit
  "intent": "purchase_api_call",                     // enum: purchase_api_call | purchase_dataset | pay_compute_job | pay_agent_service | tip
  "amount": { "value": "0.05", "currency": "USD" }, // canonical USD; vault prepočíta na token
  "token": "USDC",                                   // požadovaný settlement token
  "destination": {                                   // KAM
    "type": "x402_endpoint",                         //   x402_endpoint | eoa | smart_account | erc20_transfer
    "url": "https://api.example.com/v1/inference",
    "method": "POST",
    "expected_recipient": "0xAbC..."                 //   ak vie agent vopred (z challenge)
  },
  "payment_protocol": "x402",                        // x402 | direct_transfer | stream
  "chain": "base",                                   // base | ethereum | polygon | arbitrum | ...
  "provider_url": "https://api.example.com",
  "x402_payload": { /* parsed challenge object */ }, // ak protocol=x402
  "expiry": "2026-04-25T10:31:00Z",
  "nonce": "01HTAWX...K9",                           // ULID alebo UUIDv7 — replay protection
  "expected_result": {
    "type": "api_response_hash",
    "schema_url": "https://api.example.com/.well-known/x402-result-schema.json"
  },
  "risk_class": "low"                                // hint od agenta: low | medium | high (vault verifikuje)
}
```

**Validácia:**
- JSON Schema strict mode (`additionalProperties: false`).
- `nonce` musí byť unikátny v rámci agent_id (replay protection).
- `expiry` musí byť v budúcnosti, max 10 minút (configurable).
- `risk_class` je iba hint, vault si urobí vlastný risk score.

**Výstup volania:** `payment_request_id` (ULID) + status (`pending` / `auto_approved` / `requires_human` / `rejected`).

---

## G.2 Policy-as-Code

**Formát:** YAML pre human-friendly definíciu, kompilácia do Rego (alebo CEL) pre evaluáciu.

**Štruktúra:**
- `agent_id` alebo `agent_group` (selektor).
- `limits` (numerické cap-y).
- `allowed` (allowlists pre protocols/chains/tokens/providers/methods).
- `approval_required` (eskalácia).
- `deny` (explicitné zákazy s vyššou prioritou než allow).
- `risk_overrides` (špeciálne pravidlá na risk level).

**Versioning:**
- Každá policy má `policy_version` (semver alebo monotonic int).
- Pri mutation sa vytvorí *nový* záznam (immutable history).
- `policy_hash = sha256(canonical_yaml)` sa uloží do audit logu.
- Aktivácia novej verzie vyžaduje admin signature (M-of-N nad threshold).

**Signed config:**
- Adminove kľúče (preferovane v HSM slot) podpíšu nový policy YAML.
- Policy engine pri load overí podpis.
- Bez platného podpisu → engine nefunguje (fail-closed).

**Žiadna zmena bez admin approval:**
- Filesystem write na `policy/` adresár chránený OS perms (vlastníctvo `mandate-admin`, `0640`).
- API mutation cez `PATCH /v1/agents/{id}/policies` vyžaduje admin JWT + ak suma threshold prekročená → M-of-N.
- Agent runtime nemá *žiadne* write priviégium na policy.

---

## G.3 Budget Ledger

**Účel:** Spoľahlivá, atomická pamäť o tom, koľko každý agent minul.

**Schéma (zjednodušená, SQLite):**

```sql
CREATE TABLE budget_accounts (
  id INTEGER PRIMARY KEY,
  agent_id TEXT NOT NULL,
  scope TEXT NOT NULL,           -- 'per_payment' | 'daily' | 'weekly' | 'monthly' | 'per_provider' | 'per_token' | 'per_task'
  scope_key TEXT,                -- e.g. 'api.example.com' for per_provider
  cap_usd NUMERIC NOT NULL,
  hard_cap BOOLEAN NOT NULL,     -- hard = absolute deny; soft = approval needed
  reset_period TEXT,             -- 'daily' | 'weekly' | 'monthly' | NULL
  current_period_start TIMESTAMP,
  current_spent_usd NUMERIC NOT NULL DEFAULT 0
);

CREATE TABLE budget_transactions (
  id INTEGER PRIMARY KEY,
  budget_account_id INTEGER NOT NULL REFERENCES budget_accounts(id),
  payment_request_id TEXT NOT NULL,
  amount_usd NUMERIC NOT NULL,
  ts TIMESTAMP NOT NULL,
  type TEXT NOT NULL  -- 'reserve' | 'commit' | 'release'
);
```

**Flow:**
1. **Reserve** — pri `auto_approved` decision sa odhad sumy zarezervuje (zvýši `current_spent_usd`).
2. **Commit** — po úspešnom on-chain confirmation alebo x402 200 OK sa rezervácia premení na commit (žiadna zmena, ale label).
3. **Release** — pri zlyhaní (x402 fail, on-chain revert) sa rezervácia uvolní (zníži `current_spent_usd`).

**Periodic reset:** cron task v rámci vaultu reštartuje `current_period_start` a `current_spent_usd` podľa `reset_period`.

**Hard vs. soft cap:**
- Hard: prekročenie → `rejected`.
- Soft: prekročenie → `requires_human`.

**Konzistencia:** všetky operácie v jednej SQLite transakcii s `BEGIN IMMEDIATE`.

---

## G.4 x402 Verifier

**Účel:** Overiť, že x402 challenge od providera je legitímny a konzistentný s tým, čo si agent objednal.

**Vstup:** parsed `x402_payload` (z HTTP 402 response headerov + body).

**Overuje:**
- **Recipient** — adresa zhoduje sa s allowlist + matching s `destination.expected_recipient`.
- **Amount** — v rámci toleranceu (default ±5 %) voči `amount.value` (po prepočte na token).
- **Asset** — token contract na chaine je v allowliste; symbol matches.
- **Network** — chain ID v challenge zhoduje sa s `chain` v requestu a v policy.
- **Endpoint** — domain v challenge je rovnaký ako pôvodný request domain (no cross-domain billing).
- **Expiry** — challenge má reasonable TTL.
- **Domain binding** — origin TLS cert pinning (cert hash uložený pri prvom kontakte alebo z policy `providers[].cert_pin`).
- **Response consistency** — challenge nesmie obsahovať polia mimo špecifikácie (ignored alebo `additionalProperties: false`).

**Výstup:** `x402_verified=true/false` + `x402_decision_payload` (čo má byť presne podpísané).

**Anti-fraud:**
- Provider reputation cache (ak >50 úspešných paymentov za posledný mesiac, nižší risk score).
- Známe legit providers môžu byť pre-curated v "default trust list" updatovanom cez signed channel.

---

## G.5 Transaction Simulator

**Účel:** Pred podpisom simulovať transakciu a porovnať očakávaný efekt s realitou.

**Implementácia:**
- `eth_call` s state override + `debug_traceCall` (alebo `cast call` cez Foundry).
- Možnosť spustiť proti rovnakému RPC ako bude broadcast (state pinning).

**Overuje:**
- Token transfer suma sa zhoduje (`balanceOf(recipient)` pred/po).
- `to` address je očakávaný contract (USDC/x402 contract).
- Calldata je expected method selector + arguments.
- Žiadne neočakávané side effects (ďalšie transfers, approve, delegatecall).
- Gas estimate v rozumnom range.
- Pri `risk_class=high` → odmietne neznáme calldata (whitelist method selectors).

**Pri mismatch:** decision = `rejected` + audit event s detailom.

**Simulácia môže byť stale:** preto pre veľmi citlivé tx použiť commit-reveal alebo same-block submission (ak chain podporuje flashbots/builders).

---

## G.6 Signing Adapter Layer

**Účel:** Abstrakcia nad signing backendmi. Vault nehovorí "podpíš HSM-om", hovorí "podpíš key handle X".

**Podporované backendy:**

| Backend | MVP | Production | Poznámka |
|---|---|---|---|
| `local_dev_key` | ✅ | ❌ | Iba dev/test, in-memory ephemeral |
| `encrypted_file` | ✅ | ⚠ low-stakes | age/sops/sodium-encrypted; passphrase/systemd cred |
| `tpm` | ⚠ | ✅ | TPM 2.0 cez `tpm2-tools` / `tabrmd`; key sealed na PCR |
| `hsm_pkcs11` | ❌ | ✅ | YubiHSM 2 / Nitrokey HSM 2 / SoftHSM cez PKCS#11 |
| `yubihsm_native` | ❌ | ✅ | Vendor SDK (lepší attestation) |
| `nitrokey_native` | ❌ | ✅ | Vendor SDK |
| `smartcard` | ❌ | ⚠ | OpenPGP card cez PKCS#11 |
| `tee_sealed` | ❌ | ✅ | TEE-derived sealing (TDX/SEV/SGX) |
| `mpc_remote` | ❌ | ⚠ | Threshold MPC ako backend (Fireblocks, Lit, Silence Labs) |
| `smart_account_session_key` | ❌ | ✅ | ERC-4337 session key signed by other backend |

**Interface (Rust trait, Go interface):**

```rust
trait SigningBackend {
    fn key_id(&self) -> KeyId;
    fn public_key(&self) -> PublicKey;
    fn sign(&self, payload: SignPayload, attestation_token: AttestationToken)
        -> Result<Signature, SignError>;
    fn attestation(&self) -> Option<AttestationEvidence>;
}
```

**Invariant:** `sign()` *vždy* overí `attestation_token` (HMAC alebo Ed25519 podpis od policy enginu) pred volaním do backend HW. Backend bez attestation tokenu odmietne.

**Konfigurácia:** YAML mapuje agentov / účely na konkrétne signing backendy:

```yaml
signing:
  default_backend: hsm_pkcs11
  keys:
    - id: agent-research-01-key
      backend: hsm_pkcs11
      slot: 0
      label: "agent-research-01"
      attestation_required: true
    - id: treasury-key
      backend: tee_sealed
      attestation_required: true
      multisig_required: true
```

**Treasury vs. operational separation:**
- *Operational key:* malý balance, denne refilovaný z treasury.
- *Treasury key:* väčší balance, vyžaduje multisig, nie je dostupný pre auto-payments.
- Refill je samostatná policy s vlastným approval flow.

---

## G.7 Attestation Layer

**Účel:** Generovať a verifikovať dôkaz, že:
1. Vault runtime beží *nezmenený* (measurement/hash kódu).
2. Loaded policy je *nezmenená* (policy_hash).
3. Signing kľúč je v dôveryhodnom HW (HSM/TEE).

**Komponenty:**

- **Runtime measurement** — pri TEE: TDX MRTD/RTMR, SEV-SNP measurement, SGX MRENCLAVE/MRSIGNER. Pri non-TEE: hash binárky + kontajner image SHA z signed manifest.
- **Policy measurement** — `sha256(canonical_policy_yaml)`.
- **Composite measurement** — `H(runtime_measurement || policy_hash || config_hash)`.
- **Attestation evidence** — TEE-specific quote/report obsahujúci composite measurement + nonce + timestamp.

**Verifikácia (off-vault):**
- Externý verifier (CLI tool, SDK, smart contract verifier) prijme evidence + nonce.
- Overí podpis cez TEE root certs (Intel DCAP, AMD SEV root, Nitro PKI).
- Porovná measurement s expected hodnotou (publikovanou v signed release manifest).

**Použitie:**
- *Off-chain:* externý monitoring service vie pravidelne pingnúť `GET /v1/attestation` a alertovať na drift.
- *On-chain:* ERC-4337 validator akceptuje user op iba ak signature obsahuje recent attestation reference (komplexné, ale možné — viď research na "TEE-attested smart account validators", napr. Automata, Phala).

**Bez TEE (V1, V2, V3 architektúra):**
- "Attestation" je iba *signed runtime claim* — vault podpíše svoju vlastnú konfiguráciu lokálnym kľúčom. Slabšie, ale stále užitočné pre detekciu lokálnych zmien.

---

## G.8 Signed Audit Log

**Účel:** Append-only, tamper-evident záznam každej dôležitej udalosti.

**Schéma záznamu:**

```jsonc
{
  "seq": 12345,
  "ts": "2026-04-25T10:30:42.123Z",
  "type": "decision_made",            // request_received | decision_made | signature_issued | policy_changed | emergency_event | ...
  "actor": "policy_engine",
  "subject_id": "pr-01HTAWX...",
  "payload_hash": "sha256:...",       // hash dôležitého payloadu (full payload nie je v logu pre privacy)
  "decision": "auto_approved",
  "policy_version": 42,
  "policy_hash": "sha256:...",
  "attestation_ref": "att-...",
  "prev_event_hash": "sha256:...",    // hash chain
  "event_hash": "sha256:...",         // sha256(canonical_serialization tohto eventu)
  "signer_pubkey": "audit-signer-pk",
  "signature": "ed25519:..."
}
```

**Hash chain:**
- `event_hash = H(canonical_event_without_signature || prev_event_hash)`.
- Verifier prejde celý log a overí každý hash.

**Daily Merkle root:**
- Z eventov daného dňa sa vytvorí Merkle tree.
- Root sa podpíše a uloží lokálne + (production) anchored on-chain (cheap L2 transaction raz denne).

**Export:**
- JSONL formát + signed manifest.
- Možnosť exportu do externého object storage (S3 with object lock, IPFS pinned, Filecoin).

**Kľúč pre podpis logu:** dedikovaný `audit-signer-key`, izolovaný od transaction signera (separation of concerns).

---

## G.9 Human Approval Gateway

**Účel:** Tam, kde policy povie "potrebujem človeka", systém čaká na ľudský vstup pred podpisom.

**Triggery:**
- Suma > policy threshold (`approval_required.if_amount_over_usd`).
- Nový recipient (nie v allowliste).
- Nový chain alebo token.
- Zmena policy (M-of-N admin podpisov).
- Prekročenie soft denného limitu.
- Neznámy x402 provider.
- Anomáliu detected (frekvencia, pattern, geo zmena).

**Notification kanály:**
- Web UI (lokálny server, push notification).
- Mobile push cez signed push service (vlastný relay; *nie* Firebase pre privacy).
- E-mail (cez lokálny SMTP relay; menej secure).
- CLI (`mandate approvals pending` → list).
- Telegram/Signal bot (optional, cez signed webhook).

**Approval payload:**
- Admin podpíše schvaľovaciu odpoveď svojim kľúčom.
- Podpis verifikovaný v policy engine.
- Audit event so signature.

**Time-to-live:**
- Default 5 minút (configurable).
- Po expiry → request automaticky `rejected`.

**Multi-approver workflow:**
- Pre treasury operations alebo policy mutations: M-of-N (napr. 2-of-3).
- Threshold mutation orchestration: *all required approvers* musia podpísať rovnaký payload do TTL.

---

## G.10 Emergency Controls

**Účel:** "Big red button" — okamžite zastaviť všetko alebo selektívne.

**Akcie:**

| Akcia | Effect | Recovery |
|---|---|---|
| `freeze_all_payments` | Vault odmieta každý payment request | `unfreeze` admin signature |
| `pause_agent <id>` | Konkrétny agent odmietnutý | `resume_agent <id>` |
| `revoke_provider <url>` | Provider odstránený z allowlist | Re-add (admin signature) |
| `revoke_recipient <addr>` | Adresa pridaná do blacklist | Remove (multisig) |
| `rotate_session_keys` | Nové session keys; staré invalidované on-chain (smart account) | Automatic (signed by admin) |
| `kill_switch` | Vault sa vypne, kľúče sealed; reštart vyžaduje admin recovery | Recovery flow |
| `export_incident_report` | Generuje signed forensic bundle (audit log slice + policy snapshot + attestation) | n/a |

**Spúšťače:**
- Manuálne (admin UI / CLI).
- Hardvérové (USB e-stop button cez `evdev` listener).
- Automatické (anomaly detection threshold prekročený).

**Hardware kill switch:**
- Voliteľný USB foot pedal alebo dedicated button.
- Pri stlačení vault okamžite enter `freeze_all` mode + signed event.

**Recovery procedure:**
- Multisig admin (M-of-N).
- Voliteľný delay window (24h) pred recovery activation, počas ktorého môžu iní admini vznesie veto.

---

## G.11 Architektonické invarianty (cross-cutting)

1. **Fail-closed.** Akékoľvek zlyhanie validácie / verifikácie → `rejected`, nikdy `silent allow`.
2. **Separation of duties.** Decision signer key ≠ transaction signer key ≠ audit signer key ≠ admin keys.
3. **Determinism.** Policy evaluation je deterministická (žiadny `time()` priamo, vstup je explicitný).
4. **Reproducibility.** Buildy reproducibilné (cargo/go reproducible builds, lockfile pinning).
5. **Observability without leakage.** Metrics a logy nikdy neobsahujú key material, plain payment payloads ani user-identifying info nad rámec audit zón.
6. **Backward-compatible policy migrations.** Stará policy verzia musí byť replayable nad starým decision logom (forensics).
