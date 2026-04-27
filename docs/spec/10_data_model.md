# J. Internal Data Model

## J.0 Princípy

- **Immutability** kde to dáva zmysel (Policy, AuditEvent, BudgetTransaction, ApprovalDecision sú append-only).
- **Hash chain** pre AuditEvent.
- **Versionovanie** pre Policy.
- **Strict referential integrity** (FK constraints).
- **Žiadny PII** v hot dátach (žiadne emaily okrem admin profile s konsentom).
- **Deterministic object IDs** — ULID/UUIDv7 (sortovateľné podľa času).
- **Agent IDs** — stable slug podľa `17_interface_contracts.md §0`; ak treba nemenný interný identifikátor, používa sa `agent_uid` (ULID).

---

## J.1 Entity

### 1. `Agent`
Predstavuje logický identitu AI agenta.

| Atribút | Typ | Popis |
|---|---|---|
| `id` | TEXT (slug) | PK, napr. `research-agent-01` |
| `agent_uid` | TEXT (ULID) | UNIQUE immutable internal id |
| `display_name` | TEXT | human-readable |
| `created_at` | TIMESTAMPTZ | |
| `created_by_admin_id` | FK → Admin | kto vytvoril |
| `status` | ENUM | active, paused, revoked |
| `current_policy_id` | FK → Policy | aktívna policy |
| `metadata` | JSONB | tagy, popis, environment |

**Vzťahy:** 1—1 `AgentIdentity` (current), 1—N `PaymentRequest`, N—1 `Policy` (current).

**Bezpečnosť:** `id` sa loguje, `metadata` nesmie obsahovať secrets (linter pri zápise).

---

### 2. `AgentIdentity`
Konkrétny credential, ktorým sa agent autentifikuje.

| Atribút | Typ | Popis |
|---|---|---|
| `id` | TEXT (ULID) | PK |
| `agent_id` | FK → Agent | |
| `type` | ENUM | mtls_cert, jwt_static, oauth2_client_credentials |
| `public_material` | TEXT | cert PEM alebo public key |
| `fingerprint` | TEXT (sha256 hex) | UNIQUE — fast lookup |
| `issued_at` | TIMESTAMPTZ | |
| `expires_at` | TIMESTAMPTZ | |
| `revoked_at` | TIMESTAMPTZ NULL | |

**Vzťahy:** N—1 `Agent`.

**Bezpečnosť:** podpísané CA-style adminom; rotation flow predpísaný.

---

### 3. `PaymentRequest`
Centrálna entita — žiadosť agenta o platbu.

| Atribút | Typ | Popis |
|---|---|---|
| `id` | TEXT (ULID) | PK = `payment_request_id` |
| `agent_id` | FK → Agent | |
| `task_id` | TEXT | logical work unit |
| `intent` | ENUM | viď APRP |
| `amount_usd` | NUMERIC(38,18) | canonical USD |
| `token` | TEXT | ticker |
| `chain` | TEXT | base/eth/polygon |
| `destination_type` | ENUM | x402_endpoint, eoa, smart_account, erc20_transfer |
| `destination_payload` | JSONB | url+method, alebo address |
| `payment_protocol` | ENUM | x402, l402, erc20_transfer, smart_account_session |
| `provider_url` | TEXT | |
| `x402_payload` | JSONB NULL | parsed challenge |
| `nonce` | TEXT | UNIQUE per agent |
| `expiry` | TIMESTAMPTZ | |
| `received_at` | TIMESTAMPTZ | |
| `status` | ENUM | received, validating, decided, awaiting_human, signing, broadcast, settled, failed, rejected |
| `decision_id` | FK → ApprovalDecision NULL | |
| `audit_root_event_id` | FK → AuditEvent | initial event |
| `request_hash` | TEXT (sha256) | canonical hash |

**Vzťahy:** N—1 `Agent`, 1—1 `ApprovalDecision`, 1—N `BudgetTransaction`, 1—N `AuditEvent`.

**Bezpečnosť:** `request_hash` sa loguje, `destination_payload` môže obsahovať URL — nie secrets.

---

### 4. `Policy`
Verziovaná policy attached na agenta alebo skupinu.

| Atribút | Typ | Popis |
|---|---|---|
| `id` | TEXT (ULID) | PK |
| `name` | TEXT | logical name |
| `version` | INTEGER | monotonic per name |
| `yaml_canonical` | TEXT | normalized form |
| `policy_hash` | TEXT (sha256) | UNIQUE |
| `compiled_rego` | TEXT | derived |
| `created_at` | TIMESTAMPTZ | |
| `created_by_admin_id` | FK → Admin | |
| `signatures` | JSONB | array of admin signatures |
| `activation_status` | ENUM | draft, active, deprecated, revoked |
| `parent_policy_id` | FK → Policy NULL | predchádzajúca verzia |

**Vzťahy:** 1—N `PaymentRequest` (cez `Agent.current_policy_id`).

**Bezpečnosť:** immutable po vytvorení; aktivácia je samostatný transition.

---

### 5. `PolicyVersion`
*Alternatívne modelovanie:* `Policy` riadok je už verzia, `PolicyVersion` je view nad rovnakou tabuľkou. Necháme ako *concept*, fyzicky nie samostatná tabuľka v MVP.

---

### 6. `BudgetAccount`
Limity priradené agentovi v rôznych scope.

| Atribút | Typ | Popis |
|---|---|---|
| `id` | TEXT (ULID) | PK |
| `agent_id` | FK → Agent | |
| `scope` | ENUM | per_payment, daily, weekly, monthly, per_provider, per_token, per_task |
| `scope_key` | TEXT NULL | pre per_provider/per_token = key |
| `cap_usd` | NUMERIC | |
| `hard_cap` | BOOLEAN | |
| `reset_period` | ENUM NULL | daily, weekly, monthly |
| `current_period_start` | TIMESTAMPTZ | |
| `current_spent_usd` | NUMERIC | invariant: ≤ cap_usd ak hard_cap |

**Vzťahy:** N—1 `Agent`, 1—N `BudgetTransaction`.

**Bezpečnosť:** updaty cez transakciu s `BEGIN IMMEDIATE`; trigger overí invariant.

---

### 7. `BudgetTransaction`
Atomická zmena budget stavu.

| Atribút | Typ | Popis |
|---|---|---|
| `id` | TEXT (ULID) | PK |
| `budget_account_id` | FK → BudgetAccount | |
| `payment_request_id` | FK → PaymentRequest | |
| `amount_usd` | NUMERIC | signed: + reserve, − release |
| `type` | ENUM | reserve, commit, release |
| `ts` | TIMESTAMPTZ | |
| `correlation_id` | TEXT | linkne reserve↔commit↔release |

**Append-only.**

---

### 8. `Provider`
Externá služba, ktorej agent platí.

| Atribút | Typ | Popis |
|---|---|---|
| `id` | TEXT | URL canonical (PK) |
| `display_name` | TEXT | |
| `cert_pin` | TEXT NULL | TLS cert hash |
| `status` | ENUM | trusted, allowed, denied, observation |
| `reputation_score` | INTEGER | rolling |
| `first_seen_at` | TIMESTAMPTZ | |
| `last_used_at` | TIMESTAMPTZ | |
| `metadata` | JSONB | |

**Vzťahy:** N—N `Policy.allowed.providers` (cez join), 1—N `PaymentRequest`.

---

### 9. `Recipient`
On-chain adresa, na ktorú smerujú platby.

| Atribút | Typ | Popis |
|---|---|---|
| `id` | TEXT (chain:address) | PK |
| `chain` | FK → ChainConfig | |
| `address` | TEXT | checksum |
| `label` | TEXT NULL | human-readable |
| `status` | ENUM | trusted, allowed, denied, blacklisted |
| `first_seen_at` | TIMESTAMPTZ | |
| `last_payment_at` | TIMESTAMPTZ | |
| `provider_id` | FK → Provider NULL | bind k providerovi |

---

### 10. `ChainConfig`
Konfigurácia podporovanej blockchain siete.

| Atribút | Typ | Popis |
|---|---|---|
| `id` | TEXT | name (base, ethereum, ...) |
| `chain_id` | INTEGER | |
| `rpc_endpoints` | JSONB | array; quorum logic |
| `simulator_endpoint` | TEXT | trace_call provider |
| `block_explorer_url` | TEXT | |
| `enabled` | BOOLEAN | |

---

### 11. `TokenConfig`
Token contract metadata.

| Atribút | Typ | Popis |
|---|---|---|
| `id` | TEXT | chain:address |
| `chain_id` | FK → ChainConfig | |
| `address` | TEXT | |
| `symbol` | TEXT | |
| `decimals` | INTEGER | |
| `is_stablecoin` | BOOLEAN | |
| `usd_oracle_ref` | TEXT NULL | price source |

---

### 12. `SigningKeyRef`
Pointer na key v signing backend (vault nepozná raw key).

| Atribút | Typ | Popis |
|---|---|---|
| `id` | TEXT (ULID) | PK |
| `display_name` | TEXT | |
| `backend_id` | FK → SigningBackend | |
| `backend_handle` | TEXT | PKCS#11 label, TPM handle, sealed file path |
| `public_key` | TEXT | hex/PEM |
| `chain_address` | TEXT | derivované (EVM address) |
| `purpose` | ENUM | operational, treasury, admin, audit, attestation |
| `attestation_required` | BOOLEAN | |
| `multisig_required` | BOOLEAN | |
| `created_at` | TIMESTAMPTZ | |
| `revoked_at` | TIMESTAMPTZ NULL | |

**Bezpečnosť:** `backend_handle` nikdy nie je secret per se, ale loguje sa iba hash.

---

### 13. `SigningBackend`
Konkrétna inštancia backendu.

| Atribút | Typ | Popis |
|---|---|---|
| `id` | TEXT | PK |
| `type` | ENUM | local_dev_key, encrypted_file, tpm, hsm_pkcs11, yubihsm_native, nitrokey_native, smartcard, tee_sealed, mpc_remote, smart_account_session |
| `config` | JSONB | type-specific |
| `health_status` | ENUM | healthy, degraded, offline |
| `last_health_check_at` | TIMESTAMPTZ | |

---

### 14. `ApprovalRequest`
Otvorená požiadavka na human approval.

| Atribút | Typ | Popis |
|---|---|---|
| `id` | TEXT (ULID) | PK |
| `payment_request_id` | FK → PaymentRequest | |
| `reason` | TEXT | čo vyvolalo escalation |
| `required_approvals` | INTEGER | M v M-of-N |
| `created_at` | TIMESTAMPTZ | |
| `expires_at` | TIMESTAMPTZ | TTL |
| `status` | ENUM | pending, approved, rejected, expired |

**Vzťahy:** 1—N `ApprovalDecision`.

---

### 15. `ApprovalDecision`
Konkrétny podpísaný approve/reject od adminstrátora.

| Atribút | Typ | Popis |
|---|---|---|
| `id` | TEXT (ULID) | PK |
| `approval_request_id` | FK → ApprovalRequest | |
| `admin_id` | FK → Admin | |
| `decision` | ENUM | approved, rejected |
| `comment` | TEXT | |
| `signature` | TEXT (Ed25519) | |
| `signed_at` | TIMESTAMPTZ | |

---

### 16. `AuditEvent`
Append-only event v hash-chained log.

| Atribút | Typ | Popis |
|---|---|---|
| `seq` | INTEGER | PK, monotonic |
| `id` | TEXT (ULID) | UNIQUE |
| `ts` | TIMESTAMPTZ | |
| `type` | ENUM | request_received, x402_verified, policy_decided, simulated, human_approval_needed, human_approved, human_rejected, decision_signed, tx_signed, tx_broadcast, settlement_complete, policy_changed, agent_created, key_rotated, emergency_freeze, emergency_resume, attestation_generated, attestation_drift, ... |
| `actor` | TEXT | system | admin_id | agent_id |
| `subject_id` | TEXT | typically payment_request_id |
| `payload_hash` | TEXT (sha256) | |
| `metadata` | JSONB | event-specific (NO secrets) |
| `policy_version` | INTEGER NULL | active policy |
| `attestation_ref` | FK → AttestationEvidence NULL | |
| `prev_event_hash` | TEXT (sha256) | |
| `event_hash` | TEXT (sha256) | UNIQUE |
| `signer_pubkey` | TEXT | audit signer |
| `signature` | TEXT (Ed25519) | |

**Constraint:** `event_hash = sha256(canonical(this without signature) || prev_event_hash)`.

---

### 17. `AttestationEvidence`

| Atribút | Typ | Popis |
|---|---|---|
| `id` | TEXT (ULID) | PK |
| `format` | ENUM | intel_tdx_quote, amd_sev_snp_report, intel_sgx_quote, nitro_doc, self_signed |
| `runtime_measurement` | TEXT | hex |
| `policy_hash` | TEXT | hex |
| `composite_measurement` | TEXT | |
| `nonce` | TEXT | |
| `evidence_blob` | BYTEA | |
| `generated_at` | TIMESTAMPTZ | |
| `expires_at` | TIMESTAMPTZ | recommended TTL |

---

### 18. `RuntimeMeasurement`
Aktuálny stav vault runtime (jeden záznam = aktuálny + history).

| Atribút | Typ | Popis |
|---|---|---|
| `id` | TEXT | PK |
| `binary_sha256` | TEXT | |
| `release_tag` | TEXT | |
| `tee_type` | ENUM NULL | |
| `tee_measurement` | TEXT NULL | |
| `loaded_at` | TIMESTAMPTZ | |

---

### 19. `EmergencyState`
Singleton (jeden riadok), reflektuje aktuálny vault state.

| Atribút | Typ | Popis |
|---|---|---|
| `id` | INTEGER | PK = 1 |
| `frozen` | BOOLEAN | |
| `frozen_at` | TIMESTAMPTZ NULL | |
| `frozen_by` | FK → Admin NULL | |
| `frozen_reason` | TEXT | |
| `paused_agents` | JSONB | array of agent_id |
| `revoked_providers` | JSONB | |

---

### 20. `IncidentReport`
Snapshot pre forensics / compliance.

| Atribút | Typ | Popis |
|---|---|---|
| `id` | TEXT (ULID) | PK |
| `created_at` | TIMESTAMPTZ | |
| `created_by_admin_id` | FK → Admin | |
| `description` | TEXT | |
| `audit_event_range` | JSONB | seq from/to |
| `policy_snapshot_id` | FK → Policy | |
| `attestation_snapshot_id` | FK → AttestationEvidence | |
| `signed_bundle_hash` | TEXT | sha256 export bundle |
| `external_refs` | JSONB | jiry/ticket/CVE links |

---

### 21. (bonus) `Admin`
Administrátor s right podpisovať mutácie.

| Atribút | Typ | Popis |
|---|---|---|
| `id` | TEXT | PK |
| `display_name` | TEXT | |
| `pubkey` | TEXT | Ed25519 |
| `pubkey_backend` | ENUM | hsm_pkcs11, yubikey, smartcard, file |
| `roles` | JSONB | array: policy_admin, emergency_admin, recovery_admin, auditor |
| `created_at` | TIMESTAMPTZ | |
| `revoked_at` | TIMESTAMPTZ NULL | |

---

## J.2 ER Diagram (textový)

```
Admin ─┬─< Policy (signed_by)
       ├─< Agent (created_by)
       ├─< ApprovalDecision
       └─< IncidentReport

Agent ─┬─< AgentIdentity
       ├─< PaymentRequest
       ├─< BudgetAccount
       └── current_policy → Policy

PaymentRequest ─┬─< BudgetTransaction
                ├─< AuditEvent (root)
                ├─< ApprovalRequest
                ├── provider → Provider
                ├── recipient → Recipient
                └── chain → ChainConfig

ApprovalRequest ──< ApprovalDecision

BudgetAccount ──< BudgetTransaction

ChainConfig ──< TokenConfig
ChainConfig ──< Recipient

SigningBackend ──< SigningKeyRef
SigningKeyRef → AuditEvent (signature events)

AuditEvent (chain) — prev_event_hash → AuditEvent (predchádzajúci)
AuditEvent ── attestation_ref → AttestationEvidence

EmergencyState — singleton
RuntimeMeasurement — append-only history
```

---

## J.3 Bezpečnostné pravidlá pre dáta

1. **Žiadny stĺpec nesmie obsahovať raw private key, seed, mnemonic.** Schema constraint + linter v CI.
2. **Audit log je append-only na DB úrovni** (trigger blokuje UPDATE/DELETE).
3. **Backup audit logu** je samostatný proces, šifrovaný backup-key (key v HSM slot odlišnom od signing).
4. **PII minimization:** žiadny e-mail/telefón v `metadata`; admin profil môže obsahovať kontakt s konsentom.
5. **Retention:** audit log min. 7 rokov (compliance), policy verzie navždy, payment requesty 7 rokov, raw payloads (mimo hash) iba 90 dní (privacy + storage).
6. **Encryption at rest:** SQLite `SQLite Encryption Extension` alebo vlastná FS-level encryption (LUKS).
7. **Schema migration:** signed migration scripts, vault verifikuje hash pred aplikáciou.
