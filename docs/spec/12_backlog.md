# Product Backlog (Phased + Parallel-Ready)

> **Strategický kontext:** Tento backlog je navrhnutý pre paralelnú implementáciu viacerými AI agentmi. Každá story má dependency declaration (`blocked_by`, `parallel_with`), assigned modules (file paths) a acceptance gate (link na demo scenár).
>
> **Prečítaj pred prácou:** `17_interface_contracts.md` (locked schemas, error codes, formats) a `16_demo_acceptance.md` (per-story acceptance demos).

---

## Fázová mapa

Phases sú **value-based** — každá fáza dodáva *fungujúcu vertikálu*, ktorá sa dá demonštrovať. Nie sú to time slots.

| Phase | Hlavná téza | Stav po fáze | Demo gate (count) |
|---|---|---|---|
| **P0** | Foundations | Repo, CI, telemetry, contracts locked | 5 |
| **P1** | Happy-path single payment | Agent → vault → encrypted-file sign → x402 mock provider → return | 13 |
| **P2** | Real policy + budget enforcement | Rego eval, multi-scope budget, deny rules | 18 |
| **P3** | Audit log + emergency controls | Hash chain, signed events, freeze, kill switch | 14 |
| **P4** | Real x402 + simulator + multi-RPC | Live Base testnet, transaction simulation, RPC quorum | 11 |
| **P5** | Hardware isolation (TPM/HSM) | YubiHSM/Nitrokey + TPM 2.0 backends; encrypted-file deprecated for prod | 10 |
| **P6** | Human approval + multi-admin governance | Web UI + CLI + push relay; M-of-N admin signatures | 14 |
| **P7** | TEE runtime + attestation | TDX/SEV-SNP confidential VM; self-signed + HW-rooted attestation | 12 |
| **P8** | On-chain integration | Smart account session keys (Safe + 4337); on-chain audit anchor; on-chain attestation verifier | 14 |
| **P9** | Marketplace + advanced integrations | MCP server, marketplace pilot, ZK proof of policy eval (stretch) | 10 |
| **P10** | Polish, docs, packaging, release | Appliance image, hardening guides, security audit | 8 |

**Total primary phase gates:** 129. Phase red-team gates a final red-team gates sú navyše povinné pre hardening podľa `16_demo_acceptance.md`.

**Current hackathon overlay:** ETHGlobal Open Agents is the primary package. Implement P0-P3 core first, then Phase OA below. ETHPrague P8/on-chain work is secondary unless explicitly selected.

---

## Konvencie

- **ID format:** `E<epic>-S<story>` (epic-story).
- **Priorita:** **P0** = blocker pre fázu, **P1** = should, **P2** = nice-to-have.
- **Estimácia:** S/M/L/XL (irrelevantné pre time, relevantné pre rozdelenie medzi agentov).
- **`blocked_by`:** ID(s) ktoré musia byť hotové pred touto.
- **`parallel_with`:** ID(s) ktoré sa dajú robiť súčasne bez konfliktu.
- **`modules`:** file paths / package names, ktoré story ovplyvňuje.
- **`accept`:** ID demo scenára z `16_demo_acceptance.md`.

---

# Phase 0 — Foundations

**Cieľ:** Pripraviť pôdu — repo, CI, telemetry, naskôr používané contracts. Žiadna business funkcionalita.
**Exit criteria:** Všetky demo gates P0-* passujú; akýkoľvek nový developer sa vie pripojiť a spustiť `cargo build`.

## E1 — Project Foundations

### E1-S1 (P0, S) — Repo bootstrap a CI
- **modules:** `/Cargo.toml`, `/.github/workflows/ci.yml`, `/rust-toolchain.toml`, `/.gitignore`
- **blocked_by:** —
- **parallel_with:** E1-S2, E1-S3
- **accept:** `D-P0-01`
- **Akceptácia:**
  - Rust workspace **starting set:** `sbo3l-core`, `sbo3l-cli`, `sbo3l-server`. **Growth path** per `17_interface_contracts.md §8` adds `sbo3l-policy`, `sbo3l-storage`, `mandate-onchain`, `sbo3l-mcp`, `mandate-push`, `mandate-zk`, `mandate-web`, `mandate-bots` v neskorších fázach.
  - `rust-toolchain.toml` pinuje konkrétnu stable verziu (aktuálne `1.84.0`).
  - GitHub Actions workflow `ci.yml`: `cargo fmt --check`, `cargo clippy -- -D warnings -D clippy::unwrap_used`, `cargo test`, `cargo audit`.
  - SHA256 release artifact pre `sbo3l-cli` na tag.
- **Implementačné poznámky:**
  - Použiť `cargo workspace.dependencies` pre verzie spoločných crates (no version drift).
  - Lockfile (`Cargo.lock`) commitnutý.
  - CI cache (`~/.cargo`, `target/`) cez `actions/cache`.
  - Pre nové crates používať feature flags na include only what's needed (per `19_knowledge_base.md §5.4` re alloy crate explosion).

### E1-S2 (P0, S) — Project licensing & governance
- **modules:** `/LICENSE`, `/CONTRIBUTING.md`, `/SECURITY.md`, `/CODE_OF_CONDUCT.md`
- **blocked_by:** —
- **parallel_with:** E1-S1, E1-S3
- **accept:** `D-P0-02`
- **Akceptácia:**
  - Apache 2.0 license file.
  - SECURITY.md s `security@<domain>` GPG key fingerprint a 90-day disclosure policy.
  - CONTRIBUTING.md vyžaduje signed commits (DCO alebo GPG).

### E1-S3 (P0, M) — Telemetry/observability scaffold
- **modules:** `/crates/sbo3l-core/src/telemetry/mod.rs`, `Cargo.toml` deps
- **blocked_by:** E1-S1
- **parallel_with:** žiadne (touches core crate)
- **accept:** `D-P0-03`
- **Akceptácia:**
  - `tracing` + `tracing-subscriber` JSON output.
  - Prometheus metrics endpoint (`/metrics`) cez `metrics-exporter-prometheus`.
  - OpenTelemetry trace export (OTLP/grpc) konfigurovateľný cez env.
  - **Zákaz** logovania payment payload bytes alebo key material — assertion v testoch.

### E1-S4 (P0, S) — Error taxonomy
- **modules:** `/crates/sbo3l-core/src/error.rs`
- **blocked_by:** E1-S1
- **parallel_with:** E1-S2, E1-S3
- **accept:** `D-P0-04`
- **Akceptácia:**
  - Top-level `Error` enum s podsekciami (`SchemaError`, `PolicyError`, `BudgetError`, `SignerError`, `AuditError`, `AttestationError`, `EmergencyError`, `ProtocolError`).
  - Mapping na RFC 7807 `problem+json`.
  - Ku každému variantu `error_code` (string), `http_status` (u16), `audit_severity` (enum).
  - Vid `17_interface_contracts.md §3` pre kompletný catalog.

### E1-S5 (P0, S) — Configuration model
- **modules:** `/crates/sbo3l-core/src/config/mod.rs`, `/examples/mandate.toml`
- **blocked_by:** E1-S1
- **parallel_with:** E1-S2, E1-S3, E1-S4
- **accept:** `D-P0-05`
- **Akceptácia:**
  - TOML config s sections: `[server]`, `[storage]`, `[signing]`, `[audit]`, `[emergency]`, `[chains]`, `[providers]`.
  - Env var override (`SBO3L__SERVER__SOCKET_PATH`).
  - Validation pri load (fail fast s konkrétnou chybou).
  - Schema dokumentovaná v `17_interface_contracts.md §1`.

---

# Phase 1 — Happy-Path Single Payment

**Cieľ:** Demonštrovať, že agent vie poslať request a vault vie vrátiť signed response. Žiadna policy logika, žiadny audit, len happy path.
**Exit criteria:** P1-* demos passujú; agent SDK posiela mock x402 platbu, vault vracia podpísaný transaction.

## E2 — Agent Payment Request Protocol (APRP)

### E2-S1 (P0, M) — JSON Schema + canonical hashing
- **modules:** `/schemas/aprp_v1.json`, `/crates/sbo3l-core/src/protocol/aprp.rs`
- **blocked_by:** E1-S1, E1-S4
- **parallel_with:** E2-S2, E3-S1
- **accept:** `D-P1-01`, `D-P1-02`
- **Akceptácia:**
  - JSON Schema 2020-12 v `/schemas/aprp_v1.json` (single source of truth).
  - Initial corpus je seednutý v `/test-corpus/aprp/`; story je hotová až po rozšírení na 50 golden + 30 adversarial vzoriek.
  - Generated Rust types cez `typify` alebo manuálny `serde::Deserialize`.
  - `additionalProperties: false` na všetkých objektoch.
  - Canonical hash funkcia (`sha256(canonical_json(x))`) deterministická naprieč jazykmi (RFC 8785 JCS).
  - Unit test: 50 golden vzoriek + 30 adversarial (extra fields, missing required, wrong types, oversize).
- **Gotchas:**
  - **JCS canonicalization je trickier než `serde_json::to_string`** — používať **`serde_json_canonicalizer`** crate (per `19_knowledge_base.md §5.1`). **NEPOUŽÍVAJ `serde_jcs`** — abandoned (last release ~2022) s known UTF-16 + number edge case bugs.
  - `rust_decimal` (NIE `bigdecimal`) pre `amount.value` parsing — schema vyžaduje string (no float precision loss). `Decimal::from_str_exact` aby silent rounding nezostal.
  - `nonce` validácia: regex `^[0-7][0-9A-HJKMNP-TV-Z]{25}$` (ULID v Crockford base32 — vid `17_interface_contracts.md §0`). **NEPOUŽÍVAJ permisívny `^[0-9A-Z]{26}$`** — ten akceptuje confusable chars (I, L, O, U).

### E2-S2 (P0, M) — Strict validator
- **modules:** `/crates/sbo3l-core/src/protocol/validator.rs`
- **blocked_by:** E2-S1
- **parallel_with:** E3-S1
- **accept:** `D-P1-03`
- **Akceptácia:**
  - Reject ak `expiry` v minulosti alebo viac než 10 minút v budúcnosti.
  - Reject ak `nonce` videný (in-memory bloom filter pre P1; persistent v P2).
  - Reject ak `agent_id` neexistuje alebo `revoked_at` not null.
  - Reject ak `chain` not in supported list (z config).
  - Reject ak `token` not in supported list.
  - Fuzz target s `cargo-fuzz` — 1h corpus run bez pádov v CI nightly.

### E2-S3 (P1, S) — Python SDK
- **modules:** `/sdks/python/mandate_client/`
- **blocked_by:** E2-S1
- **parallel_with:** E2-S4, E3-*
- **accept:** `D-P1-04`
- **Akceptácia:**
  - `pip install mandate-client`.
  - `from mandate_client import VaultClient` — `client.request_payment(...)` blocking + `arequest_payment` async.
  - Type hints generated z JSON schema.
  - Example agent v 20 riadkoch v `/sdks/python/examples/simple_agent.py`.

### E2-S4 (P1, S) — TypeScript SDK
- **modules:** `/sdks/typescript/`
- **blocked_by:** E2-S1
- **parallel_with:** E2-S3
- **accept:** `D-P1-05`
- **Akceptácia:**
  - `npm i @mandate/client`.
  - Generated types from JSON schema cez `json-schema-to-typescript`.
  - Promise-based API.

## E3 — Gateway & Auth (Phase 1 subset)

### E3-S1 (P0, M) — Unix socket + TCP loopback listener
- **modules:** `/crates/sbo3l-core/src/server/transport.rs`
- **blocked_by:** E1-S1, E1-S5
- **parallel_with:** E2-S1
- **accept:** `D-P1-06`
- **Akceptácia:**
  - Default Unix socket `/run/sbo3l/sbo3l.sock` s perms `0600`, owner `mandate`.
  - TCP loopback `127.0.0.1:8730` (configurable, default disabled v production).
  - Identický REST API behind oboma transportami.
  - Graceful shutdown s SIGTERM (drain in-flight requests s 30s timeout).
- **Gotchas:**
  - Pri Unix socket — kontrolovať peer credentials (`SO_PEERCRED` na Linuxe) pre baseline auth.
  - Pri TCP — *nikdy* nebindovať na 0.0.0.0; explicit assertion v config validátore.

### E3-S2 (P0, M) — mTLS pre agentov
- **modules:** `/crates/sbo3l-core/src/server/auth/mtls.rs`, `/crates/sbo3l-cli/src/admin/agent.rs`
- **blocked_by:** E3-S1
- **parallel_with:** E3-S4, E3-S5
- **accept:** `D-P1-07`
- **Akceptácia:**
  - Vault drží private CA (file pre P1, HSM v P5).
  - `mandate admin agent create --id research-01 --csr csr.pem` vydá cert s `CN=research-01`, TTL 30d default.
  - mTLS handshake na TCP transport — peer cert je extracted, `agent_id = CN`.
  - Cert revocation list (`crl.pem`) auto-reload pri zmene súboru.
- **Gotchas:**
  - **Cert pinning na agent strane** — vaultov server cert musí byť rovnako overený, inak útočník vie podstrčiť falošný vault.
  - SAN (Subject Alternative Name) musí obsahovať vault hostname + IP.

### E3-S4 (P0, S) — Rate limiting per-agent
- **modules:** `/crates/sbo3l-core/src/server/rate_limit.rs`
- **blocked_by:** E3-S2
- **parallel_with:** E3-S5
- **accept:** `D-P1-08`
- **Akceptácia:**
  - Token bucket per agent_id, config `requests_per_minute` (default 30).
  - Po prekročení: HTTP 429 + `Retry-After` header.
  - Counter metric `mandate_rate_limit_exceeded_total{agent_id}`.

## E8 — Signing Adapter Layer (Phase 1 subset)

### E8-S1 (P0, M) — `local_dev_key` + `encrypted_file` backends
- **modules:** `/crates/sbo3l-core/src/signing/backend/{local_dev,encrypted_file}.rs`, `/crates/sbo3l-core/src/signing/mod.rs` (trait)
- **blocked_by:** E1-S4, E1-S5
- **parallel_with:** E2-*, E3-*
- **accept:** `D-P1-09`, `D-P1-10`, `D-P1-11`
- **Akceptácia:**
  - `SigningBackend` trait s metódami `key_id()`, `public_key()`, `sign(payload, attestation_token)`, `attestation()`.
  - `local_dev_key` — ephemeral secp256k1 key generovaný pri startupe; iba ak `config.signing.allow_dev_key=true`.
  - `encrypted_file` — age-encrypted secp256k1 key; passphrase z `systemd-creds` alebo `SBO3L_PASSPHRASE` env (with warning log).
  - Sign produces standard Ethereum signature (65 bytes, recoverable v).
  - **Test vault key ≠ production key** — config schema vyžaduje explicit `is_test: true` pre `local_dev_key`.
- **Gotchas:**
  - `secp256k1` crate vs `k256` — preferovať `k256` (RustCrypto, audited, no FFI).
  - Lock file v memory (`mlock`) aby key material nešiel do swap-u.
  - `age` decryption: passphrase nesmie byť uložená v procese po use; zero-out po decrypt.

### E8-S5 (P0, M) — Internal decision-token verification v signer
- **modules:** `/crates/sbo3l-core/src/signing/decision_token.rs`
- **blocked_by:** E8-S1
- **parallel_with:** E2-*, E3-*
- **accept:** `D-P1-12`
- **Akceptácia:**
  - Decision token = Ed25519 signature (alebo HMAC-SHA256 v dev mode) nad canonical decision payload.
  - Signer **odmietne** sign request bez validného decision tokenu — error `SignerError::MissingDecisionToken`.
  - Decision signing key ≠ transaction signing key (separation enforced v config validation).
  - Red-team test: agent sa pokúsi obísť policy a zavolať priamo signer endpoint → reject.

## E16 — Integrations (Phase 1 subset — mock x402)

### E16-S0 (P0, M) — Lokálny mock x402 provider
- **modules:** `/tools/mock-x402-server/`
- **blocked_by:** E1-S1
- **parallel_with:** všetko ostatné v P1
- **accept:** `D-P1-13`
- **Akceptácia:**
  - Standalone Rust binary `mock-x402-server`.
  - Vracia HTTP 402 s validnou x402 v2 challenge pre každý GET/POST na `/api/*`.
  - **Headers:** Implementuj **OBE** sady headerov (per `19_knowledge_base.md §2.2`):
    - **x402 v2 (preferred):** `PAYMENT-REQUIRED` (server→client), `PAYMENT-SIGNATURE` (client→server retry), `PAYMENT-RESPONSE` (server→client 200) — všetko base64-encoded JSON.
    - **x402 v1 (legacy):** `X-PAYMENT`, `X-PAYMENT-RESPONSE` — Coinbase SDK still uses these.
    - Server akceptuje oba; pri response posiela v2 ale s v1 fallback na request basis.
  - Po prijatí valid `PAYMENT-SIGNATURE` (alebo `X-PAYMENT`) header vráti 200 + JSON response.
  - Configurable port, TLS s self-signed cert (pre cert-pin testy).
  - Príklad use cases: `/api/inference`, `/api/dataset`, `/api/compute-job`.
- **Prečo P0:** bez tohto je P1 demo závislá na external x402 servery (možný downtime, zmena API).

---

# Phase 2 — Real Policy + Budget Enforcement

**Cieľ:** Vault skutočne rozhoduje, nie len podpisuje. Policy YAML → Rego → eval → decision. Budget ledger s atomickými reserve/commit/release.
**Exit criteria:** P2 demos passujú; všetky útoky D-P2-RT-* (red-team) sú odmietnuté correctly.

## E4 — Policy Engine

### E4-S1 (P0, L) — Rego embed + YAML→Rego compiler
- **modules:** `/crates/sbo3l-policy/src/{compiler,evaluator,yaml_schema}.rs`
- **blocked_by:** E1-S4
- **parallel_with:** E5-S1, E5-S2
- **accept:** `D-P2-01`, `D-P2-02`, `D-P2-03`
- **Akceptácia:**
  - YAML schema definovaná v `/schemas/policy_v1.json`; field-by-field validátor.
  - Compiler emituje Rego module (text), uložený v DB ako derived value.
  - Evaluator embedded cez `regorus` crate (zero CGO/Go dep).
  - Eval P99 < 50 ms na 30-rule policy + medium-size budget state.
  - 30+ unit test scenárov pokrývajúcich každý field v policy DSL.
- **Gotchas:**
  - **Rego non-determinism** — žiadne `time.now_ns()`, `random()`, `http.send()` v compiled policy. Whitelist Rego built-ins v compiler.
  - YAML order-sensitive (allow vs deny) — canonicalize pred hashing.
  - Edge case: prázdny `allowed.providers` → deny all (fail-closed); explicit test.

### E4-S2 (P0, M) — Policy storage + versioning
- **modules:** `/crates/sbo3l-storage/src/policy.rs`, `/migrations/V002__policy.sql`
- **blocked_by:** E4-S1, E1-S5
- **parallel_with:** E4-S3
- **accept:** `D-P2-04`
- **Akceptácia:**
  - SQLite tabuľka `policy` (per `10_data_model.md §J.1.4`).
  - Insert vytvorí novú verziu (parent_policy_id pointer); update zakázaný (trigger).
  - `GET /v1/policies/{name}/{version}` vráti exact version.
  - Replay test: dotaz `evaluate(request, policy_version=N)` dáva rovnaký výsledok bez ohľadu na aktuálnu verziu.

### E4-S3 (P0, M) — Admin signature verification (single + M-of-N)
- **modules:** `/crates/sbo3l-core/src/governance/admin_sig.rs`
- **blocked_by:** E4-S2
- **parallel_with:** —
- **accept:** `D-P2-05`, `D-P2-06`
- **Akceptácia:**
  - Ed25519 signatures verified pri policy load.
  - M-of-N config per policy (default 1; threshold pre treasury policies 2-of-3).
  - Loader **odmietne** policy bez requisite podpisov (error `PolicyError::InsufficientSignatures`).
  - Signature replay protection: `policy_hash` + `nonce` v signed payload.

### E4-S4 (P1, M) — Policy lint CLI
- **modules:** `/crates/sbo3l-cli/src/lint.rs`
- **blocked_by:** E4-S1
- **parallel_with:** E4-S5
- **accept:** `D-P2-07`
- **Akceptácia:**
  - `sbo3l policy lint policy.yaml` vracia exit code 0/1.
  - Detekuje 10+ problémov z `11_policy_model.md §K.6`:
    1. `cap_usd: 0` bez `hard_cap: true`
    2. Empty `allowed.providers` bez `extends`
    3. `ttl_seconds` mimo [60, 3600]
    4. Duplicate recipient/provider entries
    5. Invalid sha256 hex/base64 v `cert_pin`
    6. Non-existent risk class v `risk_overrides`
    7. Sum `per_provider_daily` > `max_daily_usd` (warning)
    8. `approval_required.if_amount_over_usd` > `limits.max_per_payment_usd` (logical contradiction)
    9. `deny.during_emergency: false` bez explicit warning suppression
    10. Token v `allowed.tokens` neexistuje v `chains`/`tokens` config

### E4-S5 (P1, M) — Dry-run nad historickými requestami
- **modules:** `/crates/sbo3l-cli/src/dry_run.rs`
- **blocked_by:** E4-S1, E4-S2, E10-S1
- **parallel_with:** E4-S4
- **accept:** `D-P2-08`
- **Akceptácia:**
  - `sbo3l policy dry-run --policy new.yaml --since 7d` vráti diff: ktoré historické rozhodnutia by sa zmenili.
  - Output: count z prev decision typu vs new decision typu, lists of differing request IDs.
  - Použité v `PATCH /v1/agents/{id}/policies?dry_run=true` endpointe.

## E5 — Budget Ledger

### E5-S1 (P0, M) — Schema + reserve/commit/release flow
- **modules:** `/crates/sbo3l-storage/src/budget.rs`, `/migrations/V003__budget.sql`
- **blocked_by:** E1-S1
- **parallel_with:** E4-S1
- **accept:** `D-P2-09`, `D-P2-10`
- **Akceptácia:**
  - SQLite tabuľky `budget_accounts`, `budget_transactions` per `10_data_model.md §J.1.6-7`.
  - Operations atomic v rámci `BEGIN IMMEDIATE` transakcie.
  - Trigger na `budget_accounts`: `current_spent_usd <= cap_usd` ak `hard_cap=true` (otherwise raise BudgetError).
  - Chaos test: 100 paralelných reserve attempts, súčet ≤ cap.
- **Gotchas:**
  - **SQLite WAL mode** — vyžaduje `PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA busy_timeout=5000` pre concurrent reads + single writer (per `19_knowledge_base.md §5.5`).
  - **`rust_decimal`** (NIE `bigdecimal`) pre USD amounts — fixed 128-bit, 28 decimal places, no allocation. `Decimal::from_str_exact` zabráni silent rounding.
  - Invariant: súčet `budget_transactions.amount_usd` per account == `current_spent_usd`. Periodic reconciliation check.
  - `BEGIN IMMEDIATE` (NIE `BEGIN DEFERRED`) — prevents read-then-write race conditions.

### E5-S2 (P0, S) — Periodic reset
- **modules:** `/crates/sbo3l-core/src/scheduler/budget_reset.rs`
- **blocked_by:** E5-S1
- **parallel_with:** E5-S3
- **accept:** `D-P2-11`
- **Akceptácia:**
  - In-process scheduler (tokio interval) checkuje každú minútu.
  - Reset pri `now() >= current_period_start + reset_period`.
  - Idempotentné — opakované volania v rámci minúty bez efektu.
  - TZ-aware — default UTC, configurable per agent.
  - Audit event `budget_reset` s pre/post sumami.

### E5-S3 (P1, M) — Per-provider/per-token/per-task scopes
- **modules:** `/crates/sbo3l-storage/src/budget.rs` (extends)
- **blocked_by:** E5-S1
- **parallel_with:** E5-S2
- **accept:** `D-P2-12`, `D-P2-13`
- **Akceptácia:**
  - Multi-scope reserve: AND-evaluation všetkých matching scopes.
  - Príklad: agent X má daily $10 + per_provider/api.example.com $5; request $6 voči api.example.com → reject (per_provider hard_cap exceeded).

## E6 — x402 Verifier (Phase 2 subset — mock provider only)

### E6-S1 (P0, L) — x402 challenge parser
- **modules:** `/crates/sbo3l-core/src/x402/parser.rs`, `/schemas/x402_v1.json`
- **blocked_by:** E2-S1
- **parallel_with:** E6-S2, E6-S3
- **accept:** `D-P2-14`
- **Akceptácia:**
  - Parser zvládne current x402 spec (Coinbase) + l402 fallback flag.
  - Corpus test: 20+ vzoriek (real + synthetic) v `/test-corpus/x402/`.
  - Reject malformed challenges s konkrétnou error reason.

### E6-S2 (P0, M) — Domain binding + cert pin
- **modules:** `/crates/sbo3l-core/src/x402/domain_check.rs`
- **blocked_by:** E6-S1
- **parallel_with:** E6-S3
- **accept:** `D-P2-15`, `D-P2-RT-01`
- **Akceptácia:**
  - TLS cert hash extracted z connection, porovnaný s `provider.cert_pin`.
  - MITM test: proxy podstrčí iný cert → reject error `X402Error::CertPinMismatch`.
- **Gotchas:**
  - Cert pin na **leaf cert** vs intermediate vs root — jasne dokumentovať; default leaf SPKI hash.

### E6-S3 (P0, M) — Amount/asset/network konsistencia
- **modules:** `/crates/sbo3l-core/src/x402/consistency.rs`
- **blocked_by:** E6-S1
- **parallel_with:** E6-S2
- **accept:** `D-P2-16`
- **Akceptácia:**
  - Amount tolerance config (default ±5 %), reject ak mimo.
  - Asset symbol matches `policy.allowed.tokens`.
  - Chain ID matches request.
  - 15+ unit test scenárov.

### E6-S4 (P1, M) — Provider reputation rolling score
- **modules:** `/crates/sbo3l-core/src/x402/reputation.rs`
- **blocked_by:** E6-S1, E10-S1
- **parallel_with:** E7-*
- **accept:** `D-P2-17`
- **Akceptácia:**
  - Rolling 30d window per provider; success/failure ratio.
  - Score [0–100]; integrated do risk_score; influence escalation rules.
  - Default new provider score = 50 (neutral).

### E2-S2-EXT (P0, S) — Persistent nonce store
- **modules:** `/crates/sbo3l-storage/src/nonce_store.rs`
- **blocked_by:** E2-S2, E5-S1
- **parallel_with:** —
- **accept:** `D-P2-18`
- **Akceptácia:**
  - Replace P1 in-memory bloom filter persistent SQLite tabuľkou (per agent_id, ts, nonce, expiry).
  - Garbage collect entries po `expiry + 1h`.
  - Replay test po vault reštarte: rovnaký nonce → reject.

---

# Phase 3 — Audit Log + Emergency Controls

**Cieľ:** Tamper-evident logging, kill switch, freeze. Žiadny on-chain anchor (P8).
**Exit criteria:** P3 demos passujú; tampering attack je detekovaný; kill switch reaguje pod 100 ms.

## E10 — Audit Log

### E10-S1 (P0, M) — Hash-chained event store
- **modules:** `/crates/sbo3l-storage/src/audit.rs`, `/migrations/V004__audit.sql`
- **blocked_by:** E1-S1, E1-S4
- **parallel_with:** E10-S2, E12-*
- **accept:** `D-P3-01`, `D-P3-02`
- **Akceptácia:**
  - SQLite tabuľka `audit_events` per `10_data_model.md §J.1.16`.
  - INSERT trigger overí `prev_event_hash` matches predchádzajúci `event_hash`; reject ak nie.
  - UPDATE/DELETE triggers raise error (write-only).
  - Verifier CLI: `sbo3l audit verify` prejde celý log, vráti `OK` alebo `TAMPER at seq=X`.
- **Gotchas:**
  - Race condition pri concurrent insert — použiť `BEGIN IMMEDIATE` + lock.
  - Event hash format definovaný v `17_interface_contracts.md §5`.
  - Audit signer key musí byť initialized pri prvom audit evente — bootstrap order matters.

### E10-S2 (P0, M) — Daily Merkle root + signed manifest
- **modules:** `/crates/sbo3l-core/src/audit/merkle.rs`
- **blocked_by:** E10-S1
- **parallel_with:** E10-S3
- **accept:** `D-P3-03`
- **Akceptácia:**
  - Cron task vytvára Merkle tree zo všetkých eventov daného dňa (UTC midnight).
  - Root podpísaný `audit-signer-key`, uložený ako `/var/lib/sbo3l/audit/manifests/YYYY-MM-DD.json`.
  - Verifier reproduce root z events, porovná, vráti pass/fail.

### E10-S3 (P1, M) — Export (JSONL/CSV) + sink (S3-compatible)
- **modules:** `/crates/sbo3l-core/src/audit/export.rs`
- **blocked_by:** E10-S1
- **parallel_with:** E10-S2
- **accept:** `D-P3-04`
- **Akceptácia:**
  - `sbo3l audit export --format jsonl --since 7d --sink s3://bucket/path` upload + signed manifest.
  - S3-compatible (works with MinIO, AWS S3, Backblaze B2).
  - Object lock if supported (compliance mode).

## E12 — Emergency Controls

### E12-S1 (P0, M) — `freeze_all` + `resume`
- **modules:** `/crates/sbo3l-core/src/emergency/state.rs`, REST handlers
- **blocked_by:** E10-S1
- **parallel_with:** E12-S2, E12-S3
- **accept:** `D-P3-05`, `D-P3-06`
- **Akceptácia:**
  - Singleton `EmergencyState` v DB (id=1).
  - `POST /v1/emergency/stop` — single admin signature; sets `frozen=true`; emits audit event.
  - Počas frozen state: každý nový payment request → reject `EmergencyError::Frozen`.
  - `POST /v1/emergency/resume` — M-of-N admin signatures (default 2-of-3); resets.
  - Latency: freeze efekt < 100 ms (čítané v rámci request handling middleware).

### E12-S2 (P0, S) — Pause agent / revoke provider / revoke recipient
- **modules:** `/crates/sbo3l-core/src/emergency/granular.rs`
- **blocked_by:** E12-S1
- **parallel_with:** E12-S3
- **accept:** `D-P3-07`, `D-P3-08`
- **Akceptácia:**
  - Granulárne actions cez admin signed API.
  - Audit event per action.
  - Per-agent pause: ostatní agenti pokračujú.

### E12-S3 (P1, M) — Hardware kill-switch (USB device)
- **modules:** `/crates/sbo3l-core/src/emergency/hw_switch.rs`
- **blocked_by:** E12-S1
- **parallel_with:** E12-S2
- **accept:** `D-P3-09`
- **Akceptácia:**
  - evdev listener na configurable device path (`/dev/input/event<N>`).
  - Pri key press event → freeze; signed audit event s `triggered_by: "hw_switch"`.
  - Linux only (Wayland/X11 agnostic, /dev/input).
- **Gotchas:**
  - Device permissions (`udev` rule needed); document v installation guide.
  - False positives — vyžadovať double-press v rámci 1s (configurable).

### E12-S4 (P1, M) — Anomaly-based auto-freeze
- **modules:** `/crates/sbo3l-core/src/emergency/anomaly.rs`
- **blocked_by:** E12-S1, E10-S1
- **parallel_with:** E12-S5
- **accept:** `D-P3-10`
- **Akceptácia:**
  - Rule-based anomaly score (suma > N×median, frequency spike, geo change).
  - Threshold prekročený → auto-freeze + alert.
  - False positive rate < 5 % na test corpus.

### E12-S5 (P2, M) — Recovery procedure (multisig + delay window)
- **modules:** `/crates/sbo3l-core/src/recovery/`
- **blocked_by:** E12-S1, E13-S2
- **parallel_with:** E12-S4
- **accept:** `D-P3-11`
- **Akceptácia:**
  - M-of-N admin signatures + 24h vetovať okno.
  - Documented runbook v `/docs/runbooks/recovery.md`.
  - Dry-run mode pre training.

## E11 — Human Approval Gateway (Phase 3 subset — CLI only)

### E11-S2 (P1, M) — CLI approvals
- **modules:** `/crates/sbo3l-cli/src/approvals.rs`
- **blocked_by:** E10-S1, E13-S1
- **parallel_with:** E12-*
- **accept:** `D-P3-12`
- **Akceptácia:**
  - `mandate approvals pending` → list of pending approval requests.
  - `mandate approvals sign <id>` → admin podpíše s key (file or HSM later).
  - Signed payload sent to vault via API.
  - TTL countdown displayed.

## E10 — extended

### E10-S5 (P0, S) — Audit event coverage assertion
- **modules:** `/crates/sbo3l-core/tests/audit_coverage.rs`
- **blocked_by:** E10-S1, E2-S2, E4-S1, E5-S1, E12-S1
- **parallel_with:** —
- **accept:** `D-P3-13`
- **Akceptácia:**
  - Test simuluje každý mutating action a verifikuje, že audit event sa zapíše.
  - List required events: `request_received`, `policy_decided`, `signature_issued`, `policy_changed`, `agent_created`, `key_rotated`, `emergency_freeze`, `emergency_resume`, `budget_reset`, `human_approved`, `human_rejected`.
  - Coverage report: každý mutating endpoint má assigned event type.

### E10-S6 (P0, S) — No-PII assertion
- **modules:** `/crates/sbo3l-core/tests/pii_lint.rs`
- **blocked_by:** E10-S1
- **parallel_with:** E10-S5
- **accept:** `D-P3-14`
- **Akceptácia:**
  - Lint test scanuje audit events za posledných 100 requestov; reject ak obsahuje email regex, phone regex, plný JWT, raw payload bytes nad 256 bajtov.

---

# Phase 4 — Real x402 + Simulator + Multi-RPC

**Cieľ:** Vault funguje proti reálnemu Base testnet; simulator overí transakcie; RPC quorum.
**Exit criteria:** P4 demos passujú; live transakcia na Base Sepolia; simulator catch positive controls.

## E7 — Transaction Simulator

### E7-S1 (P0, L) — eth_call + traceCall integrácia
- **modules:** `/crates/sbo3l-core/src/simulator/`
- **blocked_by:** E1-S5
- **parallel_with:** E7-S2, E16-S1
- **accept:** `D-P4-01`, `D-P4-02`
- **Akceptácia:**
  - JSON-RPC client (`alloy` crate alebo `ethers-rs`) callsuje `eth_call` + `debug_traceCall` (whichever available).
  - State pinning (block number) — simulácia a broadcast používajú rovnaký block.
  - USDC transfer simulation: vráti expected balance change; test deny ak balance change nesedí s `request.amount`.
- **Gotchas:**
  - `debug_traceCall` nie je na všetkých RPC providers — Alchemy/QuickNode áno, public RPC často nie. Fallback na `eth_call` + manual decode.
  - Block reorg behind simulator's pinned block — handle gracefully.

### E7-S2 (P0, M) — Method selector whitelist
- **modules:** `/crates/sbo3l-core/src/simulator/whitelist.rs`
- **blocked_by:** E7-S1
- **parallel_with:** E7-S3
- **accept:** `D-P4-03`, `D-P4-RT-02`
- **Akceptácia:**
  - Per `policy.allowed.contract_methods[token]`, accept iba whitelisted method selectors.
  - Direct test: agent skúsi `approve(...)` namiesto `transfer(...)` → reject.

### E7-S3 (P1, M) — Multi-RPC quorum
- **modules:** `/crates/sbo3l-core/src/simulator/quorum.rs`
- **blocked_by:** E7-S1
- **parallel_with:** E7-S2
- **accept:** `D-P4-04`
- **Akceptácia:**
  - N-of-M RPC quorum (default 2-of-3).
  - Chaos test: jeden RPC vráti odlišný state → reject `SimulatorError::QuorumDisagreement`.
  - Configurable per-chain.

## E16 — Integrations (Phase 4 subset — Base testnet)

### E16-S1 (P0, M) — Base Sepolia + USDC end-to-end
- **modules:** `/crates/sbo3l-core/src/chains/base.rs`, `/examples/base-sepolia-x402/`
- **blocked_by:** E7-S1, E6-*, E8-S1
- **parallel_with:** —
- **accept:** `D-P4-05`, `D-P4-06`
- **Akceptácia:**
  - Konfigurácia pre Base Sepolia (chain_id=84532, USDC contract).
  - Live x402 payment cez 3rd-party provider (alebo náš mock so real settlement).
  - Vault vykoná full flow: parse → policy → simulate → sign → broadcast → confirm.
  - Audit event obsahuje `tx_hash` po confirmation.

## E10 — extended

### E10-S7 (P1, S) — RPC health check + quarantine
- **modules:** `/crates/sbo3l-core/src/chains/rpc_health.rs`
- **blocked_by:** E7-S3
- **parallel_with:** —
- **accept:** `D-P4-07`
- **Akceptácia:**
  - Per-RPC health check (latency, last block, agreement with quorum).
  - Quarantine flag — quarantined RPC neúčastní quorum kým nepasoval N po sebe idúcich healtch checkov.
  - Metric `mandate_rpc_quarantined{rpc_url}`.

## E2 — extended

### E2-S5 (P0, S) — Settlement state tracking
- **modules:** `/crates/sbo3l-storage/src/settlement.rs`
- **blocked_by:** E5-S1, E10-S1
- **parallel_with:** —
- **accept:** `D-P4-08`
- **Akceptácia:**
  - Po sign + broadcast, settlement watcher monitoruje tx confirmation.
  - Audit event `settlement_complete` alebo `settlement_failed`.
  - Failed settlement → budget release.
  - Confirmation depth configurable (default 3 blocks).

### E2-S6 (P1, M) — Idempotency keys
- **modules:** `/crates/sbo3l-core/src/server/idempotency.rs`
- **blocked_by:** E2-S2-EXT
- **parallel_with:** E2-S5
- **accept:** `D-P4-09`
- **Akceptácia:**
  - `Idempotency-Key` header (per RFC draft) — duplicitná request s rovnakým key vráti pôvodný response.
  - TTL: 24h.

### E16-S2 (P1, M) — Polygon + Arbitrum support
- **modules:** `/crates/sbo3l-core/src/chains/{polygon,arbitrum}.rs`
- **blocked_by:** E16-S1
- **parallel_with:** E7-*
- **accept:** `D-P4-10`, `D-P4-11`
- **Akceptácia:**
  - Konfigurácia pre Polygon mainnet (137) a Arbitrum One (42161).
  - E2E test pre každú chain.

## E10 — extended (RT)

### E10-S8 (P0, M) — Replay protection extended
- **modules:** `/crates/sbo3l-core/src/protocol/replay.rs`
- **blocked_by:** E2-S2-EXT, E10-S1
- **parallel_with:** —
- **accept:** `D-P4-RT-03`
- **Akceptácia:**
  - Replay attack test: zachytenie validného request → resend → reject (nonce already seen).
  - Cross-restart: nonce store je persistent, replay po reštarte tiež reject.

---

# Phase 5 — Hardware Isolation

**Cieľ:** Production-grade signing — TPM 2.0, YubiHSM 2, Nitrokey HSM 2 backends.
**Exit criteria:** P5 demos passujú; encrypted-file backend označený ako `dev-only` v config validátore (production must use HW backend).

## E8 — extended

### E8-S2 (P0, L) — PKCS#11 backend (YubiHSM 2 + Nitrokey HSM 2)
- **modules:** `/crates/sbo3l-core/src/signing/backend/pkcs11.rs`
- **blocked_by:** E8-S1, E8-S5
- **parallel_with:** E8-S3
- **accept:** `D-P5-01`, `D-P5-02`
- **Akceptácia:**
  - `cryptoki` Rust crate.
  - Tested na YubiHSM 2 (vendor PKCS#11 module) + Nitrokey HSM 2 (OpenSC) + SoftHSM (CI).
  - secp256k1 / k256 keypair generation in HSM (key never extractable).
  - Sign produces Ethereum-compatible signature.
  - Per-key constraints: sign-only, no extract.

### E8-S3 (P1, L) — TPM 2.0 backend
- **modules:** `/crates/sbo3l-core/src/signing/backend/tpm.rs`
- **blocked_by:** E8-S1, E8-S5
- **parallel_with:** E8-S2
- **accept:** `D-P5-03`, `D-P5-04`
- **Akceptácia:**
  - `tss-esapi` Rust crate (Intel TSS2 binding); requires `libtss2-dev ≥ 3.2` build-time.
  - **Key sealed na PCR 7 + 11 + 14** (Secure Boot state + Unified Kernel Image + MOK), per `19_knowledge_base.md §6.4`. **NIKDY PCR 0 alebo PCR 2** — firmware update by rebricknul setup.
  - Negative test: ukradnutý disk → key sa neodomkne (PCR mismatch).
- **Gotchas:**
  - Authorization sessions nie sú `Send` — use one `Context` per thread.
  - Sealing under SRK without persistent handle survives reboot, ale PCR values reset.

### E8-S6 (P0, S) — Backend health monitor
- **modules:** `/crates/sbo3l-core/src/signing/health.rs`
- **blocked_by:** E8-S2 || E8-S3
- **parallel_with:** —
- **accept:** `D-P5-05`
- **Akceptácia:**
  - Per-backend health check každú minútu.
  - Status `healthy`, `degraded`, `offline`.
  - Pri offline → vault refuses signing requests s clear error.
  - Metric `mandate_signer_backend_status{backend, status}`.

### E8-S7 (P0, S) — Production config validator
- **modules:** `/crates/sbo3l-core/src/config/production_lint.rs`
- **blocked_by:** E8-S2, E8-S3
- **parallel_with:** —
- **accept:** `D-P5-06`
- **Akceptácia:**
  - Pri `mandate start --production` (alebo config flag `production: true`) reject ak:
    - Signing backend = `local_dev_key` alebo `encrypted_file`
    - Listener TCP exposed beyond 127.0.0.1
    - `attestation_required: false` na operational keys
    - Multisig disabled na treasury keys
  - CLI `mandate config check --production` štandalone test.

## E13 — Admin & Governance (Phase 5 subset)

### E13-S1 (P0, M) — Admin enrollment flow
- **modules:** `/crates/sbo3l-cli/src/admin/enroll.rs`
- **blocked_by:** E4-S3
- **parallel_with:** —
- **accept:** `D-P5-07`
- **Akceptácia:**
  - Bootstrap admin (single, počas init): `mandate init --admin-pubkey <hex>`.
  - Subsequent admin add: M-of-N existujúcich admins podpíše enrollment payload.
  - Setup wizard `mandate init --interactive`.

### E13-S2 (P1, M) — Multi-admin (M-of-N) policy mutations
- **modules:** `/crates/sbo3l-core/src/governance/multisig.rs`
- **blocked_by:** E13-S1, E4-S3
- **parallel_with:** —
- **accept:** `D-P5-08`
- **Akceptácia:**
  - Signature aggregation pre policy mutations nad threshold.
  - Partial signature state v `pending_mutations` table.
  - 2-of-3 e2e test.

## E16 — extended (Phase 5)

### E16-S5 (P1, M) — MCP server adapter
- **modules:** `/crates/sbo3l-mcp/`
- **blocked_by:** E2-S1, E3-S1
- **parallel_with:** E13-*, E8-*
- **accept:** `D-P5-09`, `D-P5-10`
- **Akceptácia:**
  - MCP tools: `payment.request`, `payment.simulate`, `payment.status`, `attestation.get`, `audit.tail`.
  - Použiteľné z Claude Desktop / iných MCP klientov.
  - Auth cez vault-issued credential.

---

# Phase 6 — Human Approval + Multi-Admin Governance UI

**Cieľ:** Web UI pre approval, push notifications, full M-of-N governance.
**Exit criteria:** P6 demos passujú; admin schvaľuje request z mobilu cez signed push.

## E11 — Human Approval Gateway (full)

### E11-S1 (P0, M) — Web UI (lokálne, loopback)
- **modules:** `/web-ui/`, `/crates/sbo3l-web/`
- **blocked_by:** E11-S2
- **parallel_with:** E11-S3
- **accept:** `D-P6-01`, `D-P6-02`
- **Akceptácia:**
  - SvelteKit alebo SolidJS (lightweight, no heavy framework).
  - Pages: dashboard, agents, policies, approvals (pending), budgets, audit log, emergency.
  - Sign approval cez WebAuthn (preferred) alebo CLI handoff (`mandate approvals sign --copy-from-clipboard`).
  - HTTPS s self-signed cert pre loopback.
- **Gotchas:**
  - **Žiadny third-party frontend assets** (no external CDN) — privacy + offline.
  - CSP striktné, no inline scripts.

### E11-S3 (P1, M) — Push notification (vlastný relay)
- **modules:** `/crates/sbo3l-push/`, `/relay-server/` (separate deployment)
- **blocked_by:** E11-S1
- **parallel_with:** E11-S4
- **accept:** `D-P6-03`
- **Akceptácia:**
  - Self-hosted relay (ntfy.sh-compatible alebo vlastný).
  - Signed webhook payload; mobile PWA decoduje + zobrazí.
  - Subscriber URL configurable per admin.

### E11-S4 (P2, S) — Telegram/Signal bot (optional)
- **modules:** `/crates/sbo3l-bots/`
- **blocked_by:** E11-S3
- **parallel_with:** —
- **accept:** `D-P6-04`
- **Akceptácia:**
  - Opt-in flag v config; default disabled.
  - Bot posiela notification + pri reply (signed) prijíma approval.

## E13 — extended

### E13-S3 (P2, M) — RBAC (auditor read-only, ...)
- **modules:** `/crates/sbo3l-core/src/governance/rbac.rs`
- **blocked_by:** E13-S1
- **parallel_with:** E11-*
- **accept:** `D-P6-05`
- **Akceptácia:**
  - Role table; JWT roles claim.
  - Endpoints check role per action.
  - Auditor role: read-only access cez audit-log + policy.

## E10 — extended

### E10-S9 (P1, M) — Webhook subscriptions
- **modules:** `/crates/sbo3l-core/src/webhooks/`
- **blocked_by:** E11-S3
- **parallel_with:** —
- **accept:** `D-P6-06`
- **Akceptácia:**
  - Subscriber registers URL + event types.
  - Vault posiela signed POST per event.
  - Retry s exponential backoff; dead-letter queue.

### E10-S10 (P1, M) — Anomaly digest weekly email
- **modules:** `/crates/sbo3l-core/src/audit/digest.rs`
- **blocked_by:** E10-S1, E12-S4
- **parallel_with:** E10-S9
- **accept:** `D-P6-07`
- **Akceptácia:**
  - Weekly summary: počet requests, denied count, anomaly events, top spending.
  - Local SMTP relay (no external email).

## E15 — Documentation (Phase 6)

### E15-S1 (P0, M) — Quickstart + threat model summary
- **modules:** `/docs/`
- **blocked_by:** P5 done
- **parallel_with:** E15-S2
- **accept:** `D-P6-08`
- **Akceptácia:**
  - `docs/quickstart.md` — 5-minute setup.
  - `docs/threat-model.md` — distilled z `04_threat_model.md`.
  - `docs/policy-authoring.md` — how to write policy.

### E15-S2 (P1, M) — Reference policy library
- **modules:** `/policies/reference/`
- **blocked_by:** E4-S4
- **parallel_with:** E15-S1
- **accept:** `D-P6-09`
- **Akceptácia:**
  - 5+ default policies: `default-deny-all`, `default-low-risk`, `default-research`, `default-trader`, `default-marketplace-buyer`.
  - Linter pass.
  - Use cases dokumentované.

### E15-S3 (P1, M) — Integration cookbooks
- **modules:** `/docs/cookbooks/`
- **blocked_by:** E2-S3, E2-S4, E16-S5
- **parallel_with:** E15-S1, E15-S2
- **accept:** `D-P6-10`
- **Akceptácia:**
  - LangChain integration cookbook + working code.
  - AutoGen integration cookbook.
  - MCP integration cookbook.

### E15-S4 (P2, M) — Security hardening guide
- **modules:** `/docs/hardening.md`, `/configs/apparmor/`, `/configs/selinux/`
- **blocked_by:** E14-S1
- **parallel_with:** —
- **accept:** `D-P6-11`
- **Akceptácia:**
  - AppArmor profile + SELinux policy.
  - systemd unit hardening directives (NoNewPrivileges, ProtectSystem, etc.).
  - CIS-style benchmark report.

## E11 — extended

### E11-S5 (P0, S) — Approval TTL enforcement
- **modules:** `/crates/sbo3l-core/src/approval/ttl.rs`
- **blocked_by:** E11-S2
- **parallel_with:** —
- **accept:** `D-P6-12`
- **Akceptácia:**
  - Approval requests s expirací TTL.
  - Po expiry → automaticky `rejected`; payment request → `rejected`.
  - Test: TTL 5s → wait 6s → check status.

### E11-S6 (P0, S) — Approval signature verification
- **modules:** `/crates/sbo3l-core/src/approval/sig_verify.rs`
- **blocked_by:** E11-S2, E13-S1
- **parallel_with:** E11-S5
- **accept:** `D-P6-13`
- **Akceptácia:**
  - Approval signature must match enrolled admin pubkey.
  - Reject ak admin not in `admins` table or revoked.
  - Test: forged signature → reject.

### E11-S7 (P0, S) — Multi-approval aggregation (for treasury ops)
- **modules:** `/crates/sbo3l-core/src/approval/aggregation.rs`
- **blocked_by:** E11-S6, E13-S2
- **parallel_with:** E11-S5
- **accept:** `D-P6-14`
- **Akceptácia:**
  - Pre M-of-N approval, vault čaká na M signatures pred execution.
  - Partial signature state visible v `mandate approvals show <id>`.

---

# Phase 7 — TEE Runtime + Attestation

**Cieľ:** Cieľová bezpečnostná postura — policy engine v TDX/SEV-SNP, attestation evidence pri každom decision, on-chain verifikovateľná.
**Exit criteria:** P7 demos passujú; vault beží v TDX VM; externý verifier validuje attestation.

## E9 — Attestation Layer

### E9-S1 (P0, M) — Self-signed attestation (non-TEE baseline)
- **modules:** `/crates/sbo3l-core/src/attestation/self_signed.rs`
- **blocked_by:** E1-S5
- **parallel_with:** E9-S2
- **accept:** `D-P7-01`
- **Akceptácia:**
  - Vault podpíše `composite_measurement = H(binary_sha256 || policy_hash || config_hash)` lokálnym `attestation-signing-key`.
  - External verifier CLI `mandate-verify attestation --evidence file.json` overí.

### E9-S2 (P1, L) — Intel TDX attestation
- **modules:** `/crates/sbo3l-core/src/attestation/tdx.rs`
- **blocked_by:** E9-S1
- **parallel_with:** E9-S3
- **accept:** `D-P7-02`, `D-P7-03`
- **Akceptácia:**
  - Intel DCAP quote generation.
  - Verifier overí cez Intel root certs.
  - E2E demo na TDX-capable HW (Intel NUC 13/14 alebo TDX cloud SKU).
- **Gotchas:**
  - DCAP attestation cache (PCCS) musí byť dostupný — buď local PCCS service alebo Intel cloud.
  - Quote size ~ 5 KB.

### E9-S3 (P2, L) — AMD SEV-SNP attestation
- **modules:** `/crates/sbo3l-core/src/attestation/sev_snp.rs`
- **blocked_by:** E9-S1
- **parallel_with:** E9-S2
- **accept:** `D-P7-04`
- **Akceptácia:**
  - SNP report + AMD root cert chain.
  - Demo na EPYC alebo Hetzner SEV-SNP node.

### E9-S4 (P0, S) — Attestation in audit events
- **modules:** `/crates/sbo3l-core/src/audit/attestation_link.rs`
- **blocked_by:** E9-S1, E10-S1
- **parallel_with:** —
- **accept:** `D-P7-05`
- **Akceptácia:**
  - Každý `decision_made` audit event obsahuje `attestation_ref` (reference na current attestation evidence).
  - Verifier vie z audit log získať attestation a overiť integrity-time.

### E9-S5 (P1, M) — Attestation drift detection
- **modules:** `/crates/sbo3l-core/src/attestation/drift.rs`
- **blocked_by:** E9-S1
- **parallel_with:** —
- **accept:** `D-P7-06`
- **Akceptácia:**
  - Periodic re-attestation (default každú hodinu).
  - Pri zmene `composite_measurement` → alert + (configurable) auto-freeze.
  - Test: zmena policy hash → drift detected.

## E8 — extended

### E8-S4 (P2, XL) — TEE-sealed signing backend
- **modules:** `/crates/sbo3l-core/src/signing/backend/tee_sealed.rs`
- **blocked_by:** E9-S2, E8-S5
- **parallel_with:** —
- **accept:** `D-P7-07`
- **Akceptácia:**
  - Key sealed v TEE-derived key (TDX measurement-bound).
  - Attestation-bound key release.
  - Negative test: key extract attempt → reject (TDX guarantees).

## E14 — Distribution & Packaging (Phase 7 subset)

### E14-S1 (P0, S) — Static binary release
- **modules:** `/.github/workflows/release.yml`
- **blocked_by:** E1-S1
- **parallel_with:** všetko
- **accept:** `D-P7-08`
- **Akceptácia:**
  - musl static binary pre Linux x86_64 + ARM64.
  - Signed (sigstore/cosign).
  - GitHub Releases automation.

### E14-S2 (P1, M) — `.deb` + `.rpm` package
- **modules:** `/packaging/deb/`, `/packaging/rpm/`
- **blocked_by:** E14-S1
- **parallel_with:** —
- **accept:** `D-P7-09`
- **Akceptácia:**
  - systemd unit (hardened).
  - Default config v `/etc/sbo3l/`.
  - `apt install` works on Ubuntu 24.04.

### E14-S3 (P1, M) — Docker compose example
- **modules:** `/examples/docker-compose/`
- **blocked_by:** E14-S1
- **parallel_with:** E14-S2
- **accept:** `D-P7-10`
- **Akceptácia:**
  - Compose file s vault + mock-x402-server + sample agent.
  - Healthchecks per service.
  - Quickstart README.

## E10 — extended

### E10-S11 (P1, M) — Forensic incident report bundle
- **modules:** `/crates/sbo3l-core/src/incident/`
- **blocked_by:** E9-S4, E10-S1
- **parallel_with:** —
- **accept:** `D-P7-11`
- **Akceptácia:**
  - `mandate incident export <id>` vytvára signed bundle: relevant audit events, policy snapshot, attestation snapshots, environment metadata.
  - Bundle hash zaznamenaný v audit log.

### E14-S5 (P0, S) — Reproducible build verification
- **modules:** `/.github/workflows/reproducible-build.yml`, `/scripts/verify-reproducible.sh`
- **blocked_by:** E14-S1
- **parallel_with:** —
- **accept:** `D-P7-12`
- **Akceptácia:**
  - CI builds binary 2× v rôznych runners; sha256 must match.
  - Public verification script (každý si overí).

---

# Phase 8 — On-Chain Integration

**Cieľ:** Smart account session keys; on-chain audit anchor; on-chain attestation verifier (the ETHPrague differentiator).
**Exit criteria:** P8 demos passujú; smart account na Base akceptuje TEE-attested user op.

## E16 — extended

### E16-S3 (P0, L) — ERC-4337 smart account session keys (Safe)
- **modules:** `/contracts/SafeAttestedModule.sol`, `/crates/sbo3l-onchain/src/safe.rs`
- **blocked_by:** E9-S1, E8-S2
- **parallel_with:** E16-S6
- **accept:** `D-P8-01`, `D-P8-02`
- **Akceptácia:**
  - Safe modul, ktorý akceptuje user op iba s validnou attestation reference.
  - Vault podpisuje session key authorization pre Safe.
  - Testnet demo (Base Sepolia).

### E16-S6 (P0, XL) — Custom ERC-4337 validator s on-chain attestation verification
- **modules:** `/contracts/AttestedValidator.sol`, `/contracts/AttestationRegistry.sol`
- **blocked_by:** E9-S2 (TDX) || E9-S3 (SEV)
- **parallel_with:** E16-S3
- **accept:** `D-P8-03`, `D-P8-04`, `D-P8-05`
- **Akceptácia:**
  - On-chain validator overí Intel DCAP quote signature (cez `Automata DCAP` library alebo vlastná implementácia).
  - Attestation registry kontrakt: oracle publikuje known-good `composite_measurement` hashes.
  - User op s nevalid attestation → reject `validateUserOp` returns failure.
  - **Toto je ETHPrague hero feature.**
- **Gotchas:**
  - DCAP verification on-chain je gas-heavy: **~4M gas s RIP-7212 precompile (Base/Arbitrum/Optimism), ~5M bez** (per `19_knowledge_base.md §1.3`). Cost na Base: $0.05–0.30. Pre ZK path (RISC Zero / SP1): **~250-400k gas** to verify SNARK + ~$0.05-0.20 in proof generation off-chain.
  - **NEPOUŽÍVAJ Ethereum L1** — verify by stál $15-60. Use Base / Arbitrum / Optimism.
  - Intel root certs musia byť aktualizovateľné on-chain (governance kontrakt).
  - **Pinuj BOTH `mrSigner` AND `mrTd`** — pinning iba `mrSigner` umožňuje útočníkovi spustiť akýkoľvek kód, ktorý Intel signoval.
  - Use Automata DCAP v1.1 (audited Trail of Bits Mar 2025).

### E16-S7 (P1, M) — On-chain audit log anchor
- **modules:** `/contracts/AuditAnchor.sol`, `/crates/sbo3l-onchain/src/anchor.rs`
- **blocked_by:** E10-S2
- **parallel_with:** E16-S3, E16-S6
- **accept:** `D-P8-06`
- **Akceptácia:**
  - Daily Merkle root publikovaný cez cheap L2 tx (Base).
  - Kontrakt emit event s root + timestamp.
  - Verifier z audit log + on-chain root vie potvrdiť integrity.
  - Cost: <$0.01 per anchor.

### E16-S8 (P1, M) — On-chain policy registry
- **modules:** `/contracts/PolicyRegistry.sol`
- **blocked_by:** E4-S2
- **parallel_with:** E16-S6
- **accept:** `D-P8-07`
- **Akceptácia:**
  - Hash policy konfigurácie publikovaný do registry.
  - Externý overuje, že vault X beží nad podpísanou policy verziou Y.
  - Compatible s Verax / EAS attestation framework.

## E15 — extended (sponsor tracks)

### E15-S5 (P0, S) — Sponsor track demo scripts
- **modules:** `/demo-scripts/sponsors/`
- **blocked_by:** E16-S3, E16-S6, E16-S7, E16-S8
- **parallel_with:** —
- **accept:** `D-P8-08`, `D-P8-09`, `D-P8-10`
- **Akceptácia:**
  - 1 demo script per target sponsor track:
    - `coinbase-x402-demo.sh` — full x402 happy path on Base.
    - `safe-attested-module.sh` — Safe modul s attestation.
    - `account-abstraction-attested-validator.sh` — vlastný 4337 validator.
    - `verax-eas-attestation.sh` — policy attestation v Verax registry.
    - `automata-tdx-attestation.sh` — TDX attestation pipeline.
  - Každý script self-contained, ručne spustiteľný počas pitchu.

### E11-S8 (P0, M) — Live attack demo harness
- **modules:** `/demo-scripts/red-team/`, `/demo-agents/research-agent/`, `/test-corpus/aprp/deny_prompt_injection_request.json`
- **blocked_by:** P3 done
- **parallel_with:** E15-S5
- **accept:** `D-P8-11`, `D-P8-12`, `D-P8-13`
- **Akceptácia:**
  - Pre-built scenarios pre live pitch:
    - `01-prompt-injection-deny.sh` — real research agent emitne škodlivý payment request → vault deny.
    - `02-policy-bypass-attempt.sh` — agent skúsi obísť policy → reject.
    - `03-key-exfil-attempt.sh` — útočník skúsi vytiahnuť key → impossible (HSM).
    - `04-tampering-detection.sh` — manuálna úprava audit log → verifier detect.
    - `05-killswitch.sh` — fyzický gombík → freeze.
  - Každý scenár pripravený s reset script (rerunnable).
  - `demo-agents/research-agent/run --scenario prompt-injection` musí fungovať deterministicky bez externých LLM/API credentials.

### E16-S9 (P1, M) — Live attestation visualization
- **modules:** `/web-ui/src/routes/attestation-monitor/`
- **blocked_by:** E9-S2, E11-S1
- **parallel_with:** E11-S8
- **accept:** `D-P8-14`
- **Akceptácia:**
  - Real-time UI ukazuje aktuálne attestation state, last verifier checks, drift alerts.
  - Vizuálny element pre pitch (porota vidí "live").

---

# Phase 9 — Marketplace + Advanced Integrations

**Cieľ:** Pilot s reálnym agent marketplace; ZK proofs (stretch).
**Exit criteria:** P9 demos passujú; marketplace pilot agent successfully purchases service from another agent.

## E16 — marketplace

### E16-S4 (P2, M) — Obolos-style marketplace pilot
- **modules:** `/examples/marketplace-buyer/`, `/examples/marketplace-seller/`
- **blocked_by:** E16-S1
- **parallel_with:** všetko v P9
- **accept:** `D-P9-01`, `D-P9-02`
- **Akceptácia:**
  - Buyer agent volá seller agent's API.
  - Seller vystaví x402 challenge.
  - Buyer cez vault zaplatí; seller vráti work product.
  - End-to-end demo s real settlement.

### E16-S10 (P2, XL) — ZK proof of correct policy evaluation (stretch)
- **modules:** `/zk-circuits/`, `/crates/sbo3l-zk/`
- **blocked_by:** E4-S1
- **parallel_with:** —
- **accept:** `D-P9-03`
- **Akceptácia:**
  - RISC Zero alebo SP1 zkVM execution policy evaluation.
  - On-chain verifier overuje ZK proof bez toho, aby revealed plný policy state.
  - Use case: privacy-preserving policy evaluation (sponsor track ZK projekty).

## E17 — Quality

### E17-S3 (P1, L) — Fuzz targets (full)
- **modules:** `/fuzz/`
- **blocked_by:** E2-S2, E6-S1, E4-S1
- **parallel_with:** —
- **accept:** `D-P9-04`
- **Akceptácia:**
  - cargo-fuzz targets pre APRP parser, x402 parser, policy compiler, decision token parser.
  - 24h corpus run bez pádov v CI nightly.

### E17-S4 (P1, L) — Internal red team exercise
- **modules:** `/docs/red-team/`
- **blocked_by:** P8 done
- **parallel_with:** E17-S3
- **accept:** `D-P9-05`
- **Akceptácia:**
  - Internal exercise per `04_threat_model.md` 25 attacks.
  - Report: každý attack — či mitigation funguje correctly.
  - Bugs filed; remediation tracked.

### E17-S1 (P0, M) — Unit test coverage
- **modules:** všetky crates
- **blocked_by:** P8 done
- **parallel_with:** E17-S2
- **accept:** `D-P9-06`
- **Akceptácia:**
  - `cargo tarpaulin` reports >80 % line coverage on critical crates (`sbo3l-core`, `sbo3l-policy`, `sbo3l-storage`).
  - CI gate: PR fails ak coverage < 80 %.

### E17-S2 (P0, M) — Integration tests
- **modules:** `/tests/integration/`
- **blocked_by:** P8 done
- **parallel_with:** E17-S1
- **accept:** `D-P9-07`
- **Akceptácia:**
  - Ephemeral SQLite + mock x402 + mock RPC.
  - Full payment flow e2e test.
  - Error path tests pre každú zónu trust boundary.

## E11 — Advanced

### E11-S9 (P1, M) — Mobile PWA approval app
- **modules:** `/mobile-pwa/`
- **blocked_by:** E11-S1, E11-S3
- **parallel_with:** E16-S4
- **accept:** `D-P9-08`
- **Akceptácia:**
  - Installable PWA (manifest + service worker).
  - Push notification reception cez vlastný relay.
  - Sign approval cez WebAuthn (mobile biometrics).

### E16-S11 (P2, M) — ENS-based agent identity
- **modules:** `/crates/sbo3l-core/src/identity/ens.rs`
- **blocked_by:** E2-S1
- **parallel_with:** —
- **accept:** `D-P9-09`
- **Akceptácia:**
  - Agent identity ako ENS subname (`research-01.myteam.eth`).
  - Vault overí ENS resolution vs registered pubkey.
  - Sponsor track fit: ENS.

### E16-S12 (P2, S) — Verax / EAS attestation publishing
- **modules:** `/crates/sbo3l-onchain/src/eas.rs`
- **blocked_by:** E16-S8
- **parallel_with:** E16-S11
- **accept:** `D-P9-10`
- **Akceptácia:**
  - Vault publikuje policy attestations cez Verax / EAS schema.
  - Sponsor track fit: Verax, EAS.

---

# Phase 10 — Polish, Docs, Packaging, Release

**Cieľ:** Príprava na release; appliance image; externý security audit.
**Exit criteria:** P10 demos passujú; v1.0.0 release.

## E14 — extended

### E14-S4 (P2, L) — Appliance image
- **modules:** `/appliance-image/`
- **blocked_by:** E14-S2
- **parallel_with:** E15-*, E18-*
- **accept:** `D-P10-01`
- **Akceptácia:**
  - Coreboot/Tianocore + minimal Linux + mandate preinstalled.
  - Bootovateľný USB image.
  - TPM enabled, encrypted disk.

### E14-S6 (P1, M) — Helm chart pre k8s deployment
- **modules:** `/helm/mandate/`
- **blocked_by:** E14-S3
- **parallel_with:** —
- **accept:** `D-P10-02`
- **Akceptácia:**
  - Helm chart deployment.
  - PVC pre persistent state.
  - HSM via DaemonSet (preconditions documented).

## E17 — Quality

### E17-S5 (P2, XL) — External security audit
- **modules:** `/audit-reports/`
- **blocked_by:** P9 done
- **parallel_with:** E14-*
- **accept:** `D-P10-03`
- **Akceptácia:**
  - Externý audit (Trail of Bits / Zellic / Cure53).
  - Report public.
  - All critical/high findings remediated pred v1.0 release.

## E18 — Business / Community

### E18-S1 (P2, M) — Open source release announcement
- **modules:** `/announcement/`
- **blocked_by:** P10 done
- **parallel_with:** —
- **accept:** `D-P10-04`
- **Akceptácia:**
  - Hacker News post + Twitter threads + Reddit.
  - Demo video (5 min).
  - Goal: 100★ first week.

### E18-S2 (P2, M) — Enterprise edition rationale
- **modules:** `/docs/editions.md`
- **blocked_by:** P10 done
- **parallel_with:** E18-S1
- **accept:** `D-P10-05`
- **Akceptácia:**
  - Feature matrix: community vs enterprise.
  - Pricing draft.

### E18-S3 (P2, L) — Hosted compliance / curated policy library subscription
- **modules:** `/business-plan/`
- **blocked_by:** E18-S2
- **parallel_with:** —
- **accept:** `D-P10-06`
- **Akceptácia:**
  - Pricing model.
  - SLA template.
  - Pilot customer LOIs.

## E15 — Final

### E15-S6 (P0, M) — Full production runbook
- **modules:** `/docs/runbooks/`
- **blocked_by:** P9 done
- **parallel_with:** —
- **accept:** `D-P10-07`
- **Akceptácia:**
  - Runbooks: install, recovery, key rotation, incident response, audit export, upgrade procedure.
  - Tested cez tabletop exercise.

### E15-S7 (P0, S) — API reference docs (auto-generated)
- **modules:** `/docs/api/`
- **blocked_by:** API stable (P5 done)
- **parallel_with:** E15-S6
- **accept:** `D-P10-08`
- **Akceptácia:**
  - `docs/api/openapi.json` je počas pre-implementation fázy ručne udržiavaný contract; po API stabilizácii sa generuje zo zdrojákov a diff musí zostať kompatibilný.
  - Hosted via mdbook alebo Rustdoc.

---

# Phase OA — Open Agents Sponsor Overlay

**Cieľ:** Pre ETHGlobal Open Agents ukazat **SBO3L** ako Open Agent Payment Firewall: policy/receipt/audit layer pred sponsor-native execution. `mandate` je jednotný technický namespace v kóde.
**Exit criteria:** `bash demo-scripts/run-openagents-final.sh` ukaze real agent, ENS identity, KeeperHub guarded execution, prompt-injection deny, policy receipt a audit proof.

## EOA — Open Agents adapters

### EOA-S1 (P0, M) — Policy receipt service
- **modules:** `/crates/sbo3l-core/src/receipts/`, `/schemas/policy_receipt_v1.json`, `/docs/api/openapi.json`
- **blocked_by:** E2-S1, E4-S1, E10-S1
- **parallel_with:** EOA-S2, EOA-S3
- **accept:** `D-OA-01`
- **Akceptácia:**
  - Allow aj deny decision vracia signed policy receipt.
  - Receipt obsahuje `agent_id`, `decision`, `deny_code`, `request_hash`, `policy_hash`, `audit_event_id`, `issued_at`, `signature`.
  - Receipt validátor odmietne zmenený `decision` alebo `policy_hash`.

### EOA-S2 (P0, S) — ENS agent identity proof
- **modules:** `/crates/sbo3l-identity/src/ens.rs`, `/demo-scripts/sponsors/ens-agent-identity.sh`
- **blocked_by:** E2-S1
- **parallel_with:** EOA-S1, EOA-S3
- **accept:** `D-OA-02`
- **Akceptácia:**
  - Demo resolver ziska alebo namockuje `research-agent.team.eth`.
  - Records: `sbo3l:agent_id`, `sbo3l:endpoint`, `sbo3l:policy_hash`, `sbo3l:audit_root`, `sbo3l:receipt_schema`.
  - Script overi, ze active sbo3l policy hash sa rovna ENS policy hash.

### EOA-S3 (P0, M) — KeeperHub guarded execution
- **modules:** `/crates/sbo3l-execution/src/keeperhub.rs`, `/demo-scripts/sponsors/keeperhub-guarded-execution.sh`
- **blocked_by:** EOA-S1
- **parallel_with:** EOA-S2, EOA-S4
- **accept:** `D-OA-03`
- **Akceptácia:**
  - Approved action is routed to KeeperHub CLI/API/MCP or faithful local mock if live API is unavailable.
  - Denied action never reaches execution layer.
  - Output shows `sbo3l_decision=allow|deny`, receipt id and execution id for allowed case.

### EOA-S4 (P1, M) — Uniswap guarded swap
- **modules:** `/crates/sbo3l-execution/src/uniswap.rs`, `/demo-scripts/sponsors/uniswap-guarded-swap.sh`, `/FEEDBACK.md`
- **blocked_by:** EOA-S1
- **parallel_with:** EOA-S3
- **accept:** `D-OA-04`
- **Akceptácia:**
  - Agent requests swap intent.
  - mandate enforces token allowlist, max notional, max slippage, quote freshness and budget.
  - Allowed swap produces receipt; denied swap shows exact deny code.
  - `FEEDBACK.md` exists if submitting to Uniswap bounty.

### EOA-S5 (P2, L) — Gensyn AXL buyer/seller payment
- **modules:** `/crates/sbo3l-execution/src/axl.rs`, `/demo-scripts/sponsors/gensyn-axl-buyer-seller.sh`
- **blocked_by:** EOA-S1, EOA-S3
- **parallel_with:** EOA-S6
- **accept:** `D-OA-05`
- **Akceptácia:**
  - Buyer and seller agents exchange a paid-service intent.
  - Payment intent is checked by mandate.
  - Receipt returns to buyer/seller path.

### EOA-S6 (P2, M) — 0G storage/plugin proof
- **modules:** `/crates/sbo3l-execution/src/zerog.rs`, `/demo-scripts/sponsors/0g-storage-proof.sh`
- **blocked_by:** EOA-S1
- **parallel_with:** EOA-S5
- **accept:** `D-OA-06`
- **Akceptácia:**
  - Policy receipt or agent passport is stored/retrieved through 0G path or faithful mock.
  - Demo frames mandate as plugin/tooling extension, not a new agent framework.

---

## Backlog summary heat-map

| Phase | Story count | P0 | P1 | P2 | Demo gates |
|---|---|---|---|---|---|
| P0 Foundations | 5 | 5 | 0 | 0 | 5 |
| P1 Happy path | 13 | 11 | 2 | 0 | 13 |
| P2 Policy + budget | 14 | 11 | 3 | 0 | 18 |
| P3 Audit + emergency | 10 | 8 | 1 | 1 | 14 |
| P4 Real x402 + sim | 10 | 7 | 3 | 0 | 11 |
| P5 HW isolation | 9 | 6 | 3 | 0 | 10 |
| P6 Approval + governance | 14 | 7 | 5 | 2 | 14 |
| P7 TEE + attestation | 10 | 5 | 2 | 3 | 12 |
| P8 On-chain | 10 | 6 | 4 | 0 | 14 |
| P9 Marketplace + ZK | 10 | 3 | 5 | 2 | 10 |
| P10 Polish + release | 8 | 3 | 2 | 5 | 8 |
| OA Open Agents Overlay | 6 | 3 | 1 | 2 | 6 |
| **Total** | **119** | **75** | **31** | **15** | **135** |

---

## Parallelization patterns (per phase)

V rámci jednej fázy je veľa stories nezávislých — môžu bežať súbežne. Mapping:

- **P0:** 5 nezávislých stories — 1 agent na story (5 paralel slots).
- **P1:** 4 nezávislé skupiny — APRP+SDK (1 agent), Gateway+Auth (1), Signing (1), Mock x402 (1) = 4 paralel slots.
- **P2:** Policy engine (1), Budget (1), x402 verifier (1) = 3 paralel slots; rate limit krížom je integration testing.
- **P3:** Audit (1), Emergency (1), Approval CLI (1) = 3 paralel slots.
- **OA:** Core receipts/API (Developer A), ENS/KeeperHub/Uniswap demos (Developer B) = 2 paralel slots.
- **P4:** Simulator (1), Chain integration (1), Settlement watcher (1) = 3 paralel slots.
- **P5:** PKCS#11 (1), TPM (1), Admin enrollment (1), MCP (1) = 4 paralel slots.
- **P6:** Web UI (1), Push relay (1), RBAC (1), Docs (1) = 4 paralel slots.
- **P7:** Self-signed attest (1), TDX (1), SEV (1), Packaging (1) = 4 paralel slots.
- **P8:** Safe module (1), Custom 4337 validator (1), Anchor (1), Registry (1), Demo scripts (1) = 5 paralel slots.
- **P9:** Marketplace (1), ZK (1), Fuzzing (1), Red team (1) = 4 paralel slots.
- **P10:** Appliance (1), Audit (1), Docs (1), Business (1) = 4 paralel slots.

Detail mapping → `18_agent_assignment.md`.

---

## Dependency graph (high-level)

```
P0 ─────► P1 ─────► P2 ─────► P3 ─────► P4 ─────► P5 ─────► P6 ─────► P7 ─────► P8 ─────► P9 ─────► P10
              │                                                          │
              └────────────────────────────────────────────────────────────┘
                  P7.E9-S2 (TDX) is prereq for P8.E16-S6 (on-chain attest verifier)
```

V rámci fázy ďalšie dependencies dokumentované v každej story (`blocked_by`).
