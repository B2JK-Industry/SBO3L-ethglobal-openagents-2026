# I. API Design

## I.0 Všeobecné konvencie

- **Base path:** `/v1` (semver — breaking changes → `/v2`).
- **Machine-readable contract:** `docs/api/openapi.json` je normatívny OpenAPI 3.1 draft pre endpointy, SDK generation a contract tests.
- **Transport:** REST/JSON nad HTTP/2 + alternatívne gRPC pre low-latency agent pripojenia.
- **Listeners:** Unix socket (default, `0600` perms), TCP loopback (`127.0.0.1:8730`), optional vsock pre VM/TEE.
- **Auth:**
  - **Agent** → mTLS s certifikátom vydaným vault adminom (CN/SAN = `agent_id`, slug regex z `17_interface_contracts.md §0`) **alebo** JWT podpísaný adminom.
  - **Admin** → mTLS s admin cert + secondary signed payload pre mutácie.
  - **Reader (audit)** → JWT s `role=audit_reader`.
- **Idempotency:** všetky `POST` mutácie akceptujú `Idempotency-Key` header (ULID/UUID); duplicitný key vráti pôvodný response.
- **Audit:** každý request **mutácia** generuje audit event (typ uvedený pri endpointe).
- **Errors:** RFC 7807 problem+json.

---

## I.1 `POST /v1/payment-requests`

**Účel:** Agent žiada o platbu.
**Auth:** mTLS (agent) — ekvivalent `cap:request:create`.
**Request body:** APRP JSON (viď `17_interface_contracts.md §2` a `schemas/aprp_v1.json`).
**Response (sync mode):**
```json
{
  "payment_request_id": "pr-01HTAWX...",
  "status": "auto_approved | requires_human | rejected | pending",
  "decision_reason": "amount within daily budget",
  "policy_version": 42,
  "estimated_completion": "2026-04-25T10:30:43Z",
  "result": {                       // ak auto_approved a hotovo
    "payment_proof": "0x...",
    "tx_hash": "0x...",
    "provider_response_ref": "..."
  }
}
```
**Async mode:** ak `Prefer: respond-async` → 202 + `payment_request_id`, agent poll `GET` alebo subscribe webhook.
**Bezpečnosť:**
- Schema validácia strict.
- Rate limit per agent (config).
- `nonce` replay protection.
**Audit:** `request_received`, `policy_decided`, neskôr `decision_signed`, `settlement_complete`.

---

## I.2 `GET /v1/payment-requests/{id}`

**Účel:** Stav konkrétneho requestu.
**Auth:** agent (vlastný request) alebo admin/auditor.
**Response:** kompletný state machine snapshot + relevant audit event refs.
**Bezpečnosť:** agent nesmie vidieť cudzie request ID.
**Audit:** žiadny (read).

---

## I.3 `POST /v1/payment-requests/{id}/approve`

**Účel:** Admin schvaľuje escalovaný request.
**Auth:** admin mTLS + signed payload (Ed25519).
**Request:**
```json
{
  "approver_id": "admin-daniel",
  "signed_decision": "ed25519:...",
  "comment": "Verified provider, OK to proceed"
}
```
**Response:** updated request status.
**Bezpečnosť:**
- Verifikuje admin signature nad canonical request hash.
- M-of-N: ak threshold prekročený, vyžaduje viacero `approve` volaní.
- TTL: po expiry `409 Conflict`.
**Audit:** `human_approved` event so signature.

---

## I.4 `POST /v1/payment-requests/{id}/reject`

**Účel:** Admin manuálne odmietne.
**Auth:** admin signed.
**Request:**
```json
{ "approver_id": "admin-daniel", "reason": "...", "signed_decision": "ed25519:..." }
```
**Response:** updated status `rejected`.
**Audit:** `human_rejected`.

---

## I.5 `GET /v1/agents`

**Účel:** Zoznam agentov a ich profilov.
**Auth:** admin / auditor.
**Response:**
```json
{
  "agents": [
    { "id": "research-agent-01", "policy_version": 42, "status": "active",
      "created_at": "...", "last_request_at": "...",
      "spent_today_usd": "1.23", "spent_month_usd": "45.6" }
  ]
}
```

---

## I.6 `POST /v1/agents`

**Účel:** Vytvorí nového agenta.
**Auth:** admin signed.
**Request:**
```json
{
  "agent_id": "trader-agent-02",
  "identity": { "type": "mtls_cert_csr", "csr_pem": "..." },
  "initial_policy_ref": "policy://default-low-risk",
  "signed_by": "admin-daniel",
  "signature": "ed25519:..."
}
```
**Response:** vault podpíše agentov client cert (CA-style) a vráti certificate + ID.
**Bezpečnosť:**
- Cert má krátku TTL (24h–30d) + auto-renew flow.
- Privátny kľúč CA pre podpisovanie cert nikdy neopustí HSM/TEE.
**Audit:** `agent_created`.

---

## I.7 `PATCH /v1/agents/{id}/policies`

**Účel:** Mení policy attached na agenta.
**Auth:** admin signed; M-of-N ak nad threshold (napr. zvýšenie limitu nad $100/deň).
**Request:**
```json
{
  "new_policy_yaml": "...",
  "policy_hash": "sha256:...",
  "signatures": [
    { "admin_id": "admin-daniel", "signature": "..." },
    { "admin_id": "admin-alex",   "signature": "..." }
  ],
  "rationale": "raise daily limit due to new use case"
}
```
**Response:** new `policy_version`, activation timestamp.
**Bezpečnosť:**
- Vault overí podpisy, porovná `policy_hash` s vlastným hashom canonical YAML.
- Ak signatures < M, vráti `requires_more_approvals` + zoznam chýbajúcich.
- Policy lint pred aktiváciou (no syntax error, no logical contradictions).
- Optional dry-run: `?dry_run=true` → vráti čo by sa zmenilo na poslednom 100 requestoch.
**Audit:** `policy_changed` + full hash chain entry.

---

## I.8 `GET /v1/budgets`

**Účel:** Aktuálny stav budget ledgers.
**Auth:** admin / auditor.
**Response:**
```json
{
  "budgets": [
    { "agent_id": "research-agent-01", "scope": "daily",
      "cap_usd": "10.00", "current_spent_usd": "1.23",
      "period_resets_at": "2026-04-26T00:00:00Z" },
    { "agent_id": "research-agent-01", "scope": "per_provider",
      "scope_key": "api.example.com", "cap_usd": "5.00", "current_spent_usd": "0.45" }
  ]
}
```

---

## I.9 `POST /v1/emergency/stop`

**Účel:** Freeze all payments.
**Auth:** admin signed; voliteľne hardware kill switch event (lokálne UID gate).
**Request:**
```json
{ "reason": "suspected compromise", "signed_by": "admin-daniel", "signature": "..." }
```
**Response:** `frozen_at` timestamp, list of in-flight requests rejected.
**Bezpečnosť:**
- Single-signature dostatočný (rýchla reakcia > opatrnosť pri zastavení).
- Resume vyžaduje multisig.
**Audit:** `emergency_freeze`.

---

## I.10 `POST /v1/emergency/resume`

**Účel:** Unfreeze.
**Auth:** admin M-of-N signed.
**Request:**
```json
{ "signatures": [ ... ], "post_incident_report_ref": "..." }
```
**Response:** `resumed_at`.
**Audit:** `emergency_resume`.

---

## I.11 `GET /v1/audit-log`

**Účel:** Stream/dotaz audit eventov.
**Auth:** auditor / admin.
**Query:**
- `since=<ulid>` cursor.
- `type=<event_type>` filter.
- `agent_id=...`.
- `format=json | jsonl | csv`.
**Response:** stream eventov + Merkle root pre verifikáciu.
**Bezpečnosť:**
- Read-only.
- Žiadny event nie je redacted (audit musí byť kompletný); citlivé payloady sú v logu len ako hashe.
**Audit:** žiadny (read), ale samotný auditor accesss môže byť meta-logged.

---

## I.12 `GET /v1/attestation`

**Účel:** Vrátiť attestation evidence pre vault runtime + active policy.
**Auth:** ktokoľvek (verifier nepotrebuje credentials), s rate limit.
**Query:** `nonce=<random_hex>` — viaže evidence na požiadavku (no replay).
**Response:**
```json
{
  "runtime_measurement": "...",
  "policy_hash": "...",
  "composite_measurement": "...",
  "evidence_format": "intel_tdx_quote | amd_sev_snp_report | nitro_attestation_doc | self_signed",
  "evidence": "base64...",
  "ts": "2026-04-25T10:30:42Z",
  "nonce": "..."
}
```
**Bezpečnosť:**
- Nonce je povinný, prevent replay.
- TEE evidence je inherentne podpísaná HW root.
- Self-signed (non-TEE variant) podpísaný `attestation-signing-key`.

---

## I.13 `GET /v1/health`

**Účel:** Liveness/readiness.
**Auth:** ktokoľvek.
**Response:** `{ "status": "ok", "version": "...", "uptime_s": ..., "warnings": [] }`.

---

## I.14 `GET /v1/runtime-model`

**Účel:** Self-popis vault runtime: aké signing backendy, aké chainy, aké protokoly, aké veľké je policy database, aké TEE/HSM je enrolled.
**Auth:** auditor / admin.
**Response:** štruktúrovaný runtime descriptor.

---

## I.15 `POST /v1/signing/simulate`

**Účel:** Pre-flight simulation — agent / admin si overí, či by request prešiel, bez podpisu.
**Auth:** agent / admin.
**Request:** APRP JSON + `simulate_only=true`.
**Response:** decision result + simulator output, ALE **bez podpisu** a **bez budget reserve commitu**.
**Bezpečnosť:**
- Použité na debugging policy.
- Stále počíta sa do rate limit.
**Audit:** `simulated_only` event (lightweight).

---

## I.16 `POST /v1/x402/verify`

**Účel:** Standalone x402 challenge verifikácia (napr. agent chce pred-overiť, či má zmysel zaplatiť).
**Auth:** agent / admin.
**Request:**
```json
{
  "challenge": { /* parsed x402 */ },
  "provider_url": "https://api.example.com",
  "expected_amount_usd": "0.05"
}
```
**Response:** verification result + structured findings.
**Audit:** lightweight `x402_verification` event.

---

## I.17 (bonus) `POST /v1/admin/keys/rotate`

**Účel:** Rotate compromised signing key (operational).
**Auth:** admin M-of-N.
**Effect:**
- HSM keygen → new key handle.
- Smart account on-chain update (ak nakonfigurované).
- Old key deactivated (no future signatures), but kept for audit verification.
**Audit:** `key_rotated`.

---

## I.18 (bonus) `GET /v1/policies/{version}`

**Účel:** Číta konkrétnu verziu policy.
**Auth:** admin / auditor.
**Response:** YAML + signatures + activation history.

---

## I.19 (bonus) Webhook `POST <subscriber_url>`

**Účel:** Push notification adminovi alebo internému monitoring systému.
**Eventy:** `human_approval_needed`, `emergency_event`, `attestation_drift`, `policy_changed`.
**Bezpečnosť:** payload signed by vault webhook key; subscriber overí.

---

## I.20 Versioning a deprecation policy

- Breaking změna → nový major prefix (`/v2`).
- Minor additívne fields → backward-compat.
- Deprecation: minimálne 6 mesiacov dual-support, `Sunset` header podľa RFC 8594.
