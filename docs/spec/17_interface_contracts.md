# Interface Contracts (Locked)

> **Účel:** Tento súbor uzamyká všetky inter-modulové kontrakty (schémy, error kódy, file paths, formaty), aby viacerí agenti mohli implementovať rôzne moduly **bez koordinácie** a ich práca do seba zapadne.
>
> **Pravidlo:** Akákoľvek zmena tohto súboru = breaking change. Musí byť explicitne odsúhlasená pred merge. Každý implementujúci agent **najprv načíta tento súbor**, potom začne kódovať.

---

## §0 Konvencie

- **Endianness:** všade little-endian, ak nie je explicit povedané inak.
- **Hash function:** SHA-256 default; v hex stringoch lowercase.
- **Time:** UTC, RFC 3339 nanosecond precision (`2026-04-25T10:30:42.123456789Z`).
- **Object IDs:** ULID (Crockford base32, 26 chars), regex `^[0-7][0-9A-HJKMNP-TV-Z]{25}$`.
- **Agent IDs:** stable human-readable slug, regex `^[a-z0-9][a-z0-9_-]{2,63}$`. Example: `research-agent-01`. If the database needs an immutable ULID, store it separately as `agent_uid`.
- **Money:** vždy string-encoded decimal, nikdy float. Príklad: `"0.05"`, nie `0.05`.
- **JSON:** UTF-8, RFC 8785 JCS (canonical) keď sa hashuje.
- **Cesty:** absolute paths, žiadne `~`.

---

## §1 Configuration Schema (`/etc/mandate/mandate.toml`)

```toml
[server]
mode = "dev" | "production"            # production reject if dev_key/encrypted_file backend
unix_socket_path = "/run/mandate/mandate.sock"
unix_socket_owner = "mandate"
unix_socket_perms = "0600"
tcp_listen = "127.0.0.1:8730"          # null to disable
http2 = true
max_request_bytes = 65536
shutdown_grace_seconds = 30

[storage]
db_path = "/var/lib/mandate/mandate.db"
wal_mode = true
journal_size_limit_mb = 64

[signing]
default_backend = "encrypted_file"     # local_dev_key | encrypted_file | tpm | hsm_pkcs11 | yubihsm_native | nitrokey_native | tee_sealed | mpc_remote | smart_account_session
allow_dev_key = false                  # production must be false
attestation_required_default = true

[[signing.keys]]
id = "agent-research-01-key"
backend = "encrypted_file"
backend_config = { path = "/var/lib/mandate/keys/agent-research-01.age" }
purpose = "operational"                # operational | treasury | admin | audit | attestation
attestation_required = true
multisig_required = false

[audit]
hash_algorithm = "sha256"
audit_signer_key_path = "/var/lib/mandate/keys/audit-signer.age"
on_chain_anchor_enabled = false
anchor_chain = "base"
anchor_period_hours = 24

[emergency]
hw_killswitch_device = "/dev/input/event5"   # null to disable
killswitch_double_press_window_ms = 1000
auto_freeze_anomaly_threshold = 0.95

[telemetry]
log_format = "json"                    # json | pretty
log_level = "info"
metrics_listen = "127.0.0.1:9091"
otlp_endpoint = "http://localhost:4317"  # null to disable

[[chains]]
id = "base"
chain_id = 8453
rpc_endpoints = [
  "https://mainnet.base.org",
  "https://base.publicnode.com",
  "https://1rpc.io/base"
]
rpc_quorum_min = 2
simulator_endpoint = "https://mainnet.base.org"
block_explorer_url = "https://basescan.org"

[[chains.tokens]]
symbol = "USDC"
address = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"
decimals = 6
is_stablecoin = true
usd_oracle_ref = null                  # null = treat as 1:1 USD

[[providers]]
id = "api.example.com"
display_name = "Example API"
cert_pin_sha256 = "AAAA...64hex..."
status = "trusted"                     # trusted | allowed | denied | observation
```

**Validácia (`mandate config check`):** must pass `production_lint` if `server.mode = "production"`.

---

## §2 Agent Payment Request Protocol (APRP) v1

### §2.1 Schema location
`/schemas/aprp_v1.json` — JSON Schema 2020-12, `$id = "https://schemas.mandate.dev/aprp/v1.json"`.

### §2.2 Canonical Rust struct

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaymentRequest {
    pub agent_id: String,                  // stable slug, see §0 Agent IDs
    pub task_id: String,                   // free-form, max 64 chars
    pub intent: Intent,                    // enum
    pub amount: Money,
    pub token: String,                     // symbol from chain.tokens
    pub destination: Destination,
    pub payment_protocol: PaymentProtocol,
    pub chain: String,                     // chain id from config
    pub provider_url: String,              // canonical URL incl. scheme
    pub x402_payload: Option<X402Payload>,
    pub expiry: chrono::DateTime<chrono::Utc>,
    pub nonce: String,                     // ULID
    pub expected_result: Option<ExpectedResult>,
    pub risk_class: RiskClass,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    PurchaseApiCall,
    PurchaseDataset,
    PayComputeJob,
    PayAgentService,
    Tip,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Money {
    pub value: String,                     // BigDecimal string, e.g. "0.05"
    pub currency: String,                  // "USD" canonical
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Destination {
    X402Endpoint { url: String, method: String, expected_recipient: Option<String> },
    Eoa { address: String },
    SmartAccount { address: String },
    Erc20Transfer { token_address: String, recipient: String },
}
```

(Plný schema je v `/schemas/aprp_v1.json`; tu je shortlist.)

### §2.3 Canonical hashing
```
request_hash = sha256(JCS-canonical-json(request_minus_signature_field))
```

JCS implementation: **`serde_json_canonicalizer`** crate (NOT `serde_jcs` — that crate is abandoned with known UTF-16/number bugs; viď `19_knowledge_base.md §5.1`).

### §2.4 Validačné pravidlá
- `expiry > now() && expiry <= now() + 10 minutes`
- `nonce` regex `^[0-7][0-9A-HJKMNP-TV-Z]{25}$` (ULID)
- `agent_id` regex `^[a-z0-9][a-z0-9_-]{2,63}$`, exists v DB a `revoked_at IS NULL`
- `chain` exists v config
- `token` exists v `chain.tokens`
- `amount.value` parsable BigDecimal, > 0, scale <= 18
- `provider_url` valid URL, scheme = `https` (allowlist `http` iba pre dev)

---

## §3 Error Catalog

Top-level enum `mandate_core::Error`. Každý variant má:

| Field | Type | Description |
|---|---|---|
| `code` | `&'static str` | machine-readable, dot-separated |
| `http_status` | `u16` | RFC 7807 mapping |
| `audit_severity` | `Severity` | `info` / `warning` / `error` / `critical` |

```rust
pub enum Error {
    // Schema / protocol
    Schema(SchemaError),
    Protocol(ProtocolError),

    // Auth
    Auth(AuthError),

    // Policy
    Policy(PolicyError),

    // Budget
    Budget(BudgetError),

    // Signer
    Signer(SignerError),

    // x402
    X402(X402Error),

    // Simulator
    Simulator(SimulatorError),

    // Audit
    Audit(AuditError),

    // Attestation
    Attestation(AttestationError),

    // Emergency
    Emergency(EmergencyError),

    // Storage
    Storage(StorageError),

    // Transport
    Transport(TransportError),
}
```

### §3.1 Komplet list error codes

| Code | Variant | HTTP | Severity |
|---|---|---|---|
| `schema.unknown_field` | SchemaError::UnknownField | 400 | warning |
| `schema.missing_field` | SchemaError::MissingField | 400 | warning |
| `schema.wrong_type` | SchemaError::WrongType | 400 | warning |
| `schema.value_out_of_range` | SchemaError::ValueOutOfRange | 400 | warning |
| `protocol.expiry_in_past` | ProtocolError::ExpiryInPast | 400 | warning |
| `protocol.expiry_too_far` | ProtocolError::ExpiryTooFar | 400 | warning |
| `protocol.nonce_replay` | ProtocolError::NonceReplay | 409 | warning |
| `protocol.unsupported_chain` | ProtocolError::UnsupportedChain | 400 | warning |
| `protocol.unsupported_token` | ProtocolError::UnsupportedToken | 400 | warning |
| `auth.no_credentials` | AuthError::NoCredentials | 401 | warning |
| `auth.invalid_credentials` | AuthError::InvalidCredentials | 401 | warning |
| `auth.agent_not_found` | AuthError::AgentNotFound | 401 | warning |
| `auth.agent_revoked` | AuthError::AgentRevoked | 401 | warning |
| `auth.rate_limited` | AuthError::RateLimited | 429 | info |
| `policy.deny_blacklisted_recipient` | PolicyError::DenyBlacklistedRecipient | 403 | error |
| `policy.deny_recipient_not_allowlisted` | PolicyError::DenyRecipientNotAllowlisted | 403 | warning |
| `policy.deny_unknown_provider` | PolicyError::DenyUnknownProvider | 403 | warning |
| `policy.deny_emergency_frozen` | PolicyError::DenyEmergencyFrozen | 423 | warning |
| `policy.deny_unknown_contract_call` | PolicyError::DenyUnknownContractCall | 403 | error |
| `policy.deny_raw_native_transfer` | PolicyError::DenyRawNativeTransfer | 403 | error |
| `policy.deny_unverified_x402` | PolicyError::DenyUnverifiedX402 | 403 | error |
| `policy.deny_simulator_disagreement` | PolicyError::DenySimulatorDisagreement | 403 | error |
| `policy.deny_attestation_drift` | PolicyError::DenyAttestationDrift | 423 | critical |
| `policy.escalation_required` | PolicyError::EscalationRequired | 202 | info |
| `policy.insufficient_signatures` | PolicyError::InsufficientSignatures | 401 | warning |
| `budget.hard_cap_exceeded` | BudgetError::HardCapExceeded | 403 | warning |
| `budget.soft_cap_warning` | BudgetError::SoftCapWarning | 202 | info |
| `signer.missing_decision_token` | SignerError::MissingDecisionToken | 401 | critical |
| `signer.invalid_decision_token` | SignerError::InvalidDecisionToken | 401 | critical |
| `signer.attestation_required` | SignerError::AttestationRequired | 401 | error |
| `signer.attestation_invalid` | SignerError::AttestationInvalid | 401 | critical |
| `signer.backend_offline` | SignerError::BackendOffline | 503 | error |
| `signer.key_not_found` | SignerError::KeyNotFound | 404 | error |
| `signer.multisig_required` | SignerError::MultisigRequired | 401 | warning |
| `x402.cert_pin_mismatch` | X402Error::CertPinMismatch | 403 | critical |
| `x402.amount_mismatch` | X402Error::AmountMismatch | 400 | error |
| `x402.asset_mismatch` | X402Error::AssetMismatch | 400 | error |
| `x402.chain_mismatch` | X402Error::ChainMismatch | 400 | error |
| `x402.expired_challenge` | X402Error::ExpiredChallenge | 400 | warning |
| `x402.malformed_challenge` | X402Error::MalformedChallenge | 400 | warning |
| `x402.domain_mismatch` | X402Error::DomainMismatch | 400 | error |
| `simulator.tx_revert` | SimulatorError::TxRevert | 400 | error |
| `simulator.balance_mismatch` | SimulatorError::BalanceMismatch | 400 | error |
| `simulator.unknown_calldata` | SimulatorError::UnknownCalldata | 403 | error |
| `simulator.quorum_disagreement` | SimulatorError::QuorumDisagreement | 503 | error |
| `simulator.rpc_unavailable` | SimulatorError::RpcUnavailable | 503 | warning |
| `audit.hash_chain_broken` | AuditError::HashChainBroken | 500 | critical |
| `audit.signer_unavailable` | AuditError::SignerUnavailable | 500 | critical |
| `audit.write_failed` | AuditError::WriteFailed | 500 | critical |
| `attestation.evidence_invalid` | AttestationError::EvidenceInvalid | 401 | critical |
| `attestation.measurement_drift` | AttestationError::MeasurementDrift | 503 | critical |
| `attestation.tee_unavailable` | AttestationError::TeeUnavailable | 503 | error |
| `emergency.frozen` | EmergencyError::Frozen | 423 | info |
| `emergency.agent_paused` | EmergencyError::AgentPaused | 423 | info |
| `emergency.recipient_blacklisted` | EmergencyError::RecipientBlacklisted | 403 | error |
| `storage.connection_failed` | StorageError::ConnectionFailed | 500 | critical |
| `storage.constraint_violation` | StorageError::ConstraintViolation | 500 | error |
| `transport.tls_handshake` | TransportError::TlsHandshake | 401 | warning |
| `transport.peer_disconnected` | TransportError::PeerDisconnected | 500 | warning |

### §3.2 RFC 7807 problem+json mapping

```json
{
  "type": "https://schemas.mandate.dev/errors/policy.deny_blacklisted_recipient",
  "title": "Recipient is blacklisted by policy",
  "status": 403,
  "detail": "Recipient 0xDEAD...beef on chain 'base' is in deny list",
  "instance": "/v1/payment-requests/pr-01HXY...",
  "code": "policy.deny_blacklisted_recipient",
  "request_id": "req-01HXY...",
  "policy_version": 42
}
```

`code` field je MANDATORY a stable across versions.

---

## §4 Decision Token Format

Decision token = signed payload, ktorý policy engine emituje a signer overí pred podpisom.

### §4.1 Payload (canonical JSON, JCS-encoded)

```json
{
  "version": 1,
  "request_hash": "<sha256 hex of full PaymentRequest>",
  "decision": "allow",
  "policy_version": 42,
  "policy_hash": "<sha256 hex of canonical policy YAML>",
  "tx_template": {
    "chain_id": 8453,
    "to": "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913",
    "value": "0",
    "data": "0xa9059cbb...",
    "gas_limit": 100000,
    "max_fee_per_gas": "1000000000",
    "max_priority_fee_per_gas": "100000000",
    "nonce_hint": null
  },
  "key_id": "agent-research-01-key",
  "decision_id": "dec-01HXY...",
  "issued_at": "2026-04-25T10:30:42.123456789Z",
  "expires_at": "2026-04-25T10:35:42.123456789Z",
  "attestation_ref": "att-01HXY..."
}
```

### §4.2 Signature
- Algorithm: Ed25519
- Signing key: `decision-signer-key` (separate from transaction signing key)
- Signature over: `sha256(JCS-canonical-json(payload))`
- Format: hex-encoded 64-byte signature

### §4.3 Wire format (passing to signer backend)

```rust
pub struct DecisionToken {
    pub payload: DecisionPayload,
    pub signature_hex: String,
    pub signing_pubkey_hex: String,    // for backend to look up trusted set
}
```

### §4.4 Verification rules (signer-side)
1. Verify signature using known `decision-signer-key` pubkey.
2. Check `expires_at > now()`.
3. Check `payload.key_id` matches the key being requested.
4. Check `request_hash` matches a prior valid request (replay protection cross-decision).
5. If `attestation_required`, dereference `attestation_ref` and verify it's valid + recent (default ≤ 60 s).

---

## §5 Audit Event Format

### §5.1 Canonical event (for hashing)

```json
{
  "version": 1,
  "seq": 12345,
  "id": "evt-01HXY...",
  "ts": "2026-04-25T10:30:42.123456789Z",
  "type": "decision_made",
  "actor": "policy_engine",
  "subject_id": "pr-01HXY...",
  "payload_hash": "<sha256 of relevant data>",
  "metadata": { "decision": "allow", "policy_version": 42 },
  "policy_version": 42,
  "policy_hash": "<sha256 hex>",
  "attestation_ref": "att-01HXY...",
  "prev_event_hash": "<sha256 hex of previous event_hash>"
}
```

### §5.2 Event hash

```
event_hash = sha256(JCS-canonical-json(canonical_event))
```

For seq=1 (genesis): `prev_event_hash = "0000...0000"` (64 zeros).

### §5.3 Signature wrapper (for storage)

```json
{
  "event": { ...canonical event... },
  "event_hash": "<sha256 hex>",
  "signature": {
    "algorithm": "ed25519",
    "key_id": "audit-signer-v1",
    "signature_hex": "..."
  }
}
```

### §5.4 Komplet list event types

```
agent_created
agent_revoked
agent_paused
agent_resumed
policy_changed
policy_revoked
key_created
key_rotated
key_revoked
request_received
request_rejected
schema_validation_failed
auth_failed
x402_verified
x402_verification_failed
policy_decided
budget_reserved
budget_committed
budget_released
budget_reset
simulated
simulation_failed
human_approval_needed
human_approved
human_rejected
human_approval_expired
decision_signed
tx_signed
tx_broadcast
settlement_complete
settlement_failed
emergency_freeze
emergency_resume
provider_revoked
recipient_blacklisted
attestation_generated
attestation_drift
hw_killswitch_triggered
incident_report_created
audit_export
audit_anchor_published
config_loaded
runtime_started
runtime_shutdown
```

---

## §6 Storage Schema Versioning

### §6.1 Migration files
- Path: `/migrations/V<NNN>__<description>.sql`
- Format: `V001__init.sql`, `V002__policy.sql`, `V003__budget.sql`, `V004__audit.sql`, ...
- Forward-only; no down migrations (production safety).
- Each migration starts with `BEGIN TRANSACTION;` ends with `COMMIT;`.
- Migration runner stores `schema_migrations(version, applied_at, sha256)` table.
- On startup, vault verifies SHA256 of each applied migration matches expected — refuses to start on mismatch.

### §6.2 Reserved tables (do not modify outside migrations)
- `schema_migrations`
- `audit_events`
- `policy`
- `budget_accounts`, `budget_transactions`
- `agents`, `agent_identities`
- `payment_requests`
- `signing_keys`
- `attestation_evidence`
- `emergency_state`
- `admins`
- `approval_requests`, `approval_decisions`
- `nonce_store`
- `providers`
- `recipients`
- `chain_configs`, `token_configs`

---

## §7 File System Layout

```
/etc/mandate/
├── mandate.toml                  # main config (0640 mandate:mandate)
├── policies/                   # active YAML policies (0640)
│   └── default.yaml
├── trusted-admins.toml         # admin pubkey roster
└── tls/
    ├── ca.crt                  # vault CA cert
    └── ca.key                  # vault CA private key (HSM-backed in prod)

/var/lib/mandate/
├── mandate.db                    # SQLite (0600 mandate:mandate)
├── mandate.db-wal
├── mandate.db-shm
├── keys/                       # encrypted keys (0600)
│   ├── audit-signer.age
│   ├── decision-signer.age
│   └── agent-research-01.age
├── audit/
│   ├── manifests/              # daily Merkle root manifests
│   │   └── 2026-04-25.json
│   └── exports/                # one-off exports
└── attestation/
    └── evidence/               # cached attestation quotes

/run/mandate/
└── mandate.sock                  # Unix socket (0600 mandate:mandate)

/var/log/mandate/
└── mandate.log                   # if not using journald

/usr/lib/systemd/system/
└── mandate.service
```

---

## §8 Crate Layout (Rust workspace)

```
/Cargo.toml                                # workspace root
/Cargo.lock
/crates/
  mandate-core/                        # library: protocol, server, signer trait, etc.
  mandate-policy/                      # library: YAML→Rego compiler + evaluator
  mandate-storage/                     # library: SQLite repositories
  mandate-onchain/                     # library: smart account + RPC client
  mandate-mcp/                         # library: MCP server adapter
  mandate-push/                        # library: push notification client
  mandate-zk/                          # library: ZK proof of policy eval (P9)
  mandate-cli/                         # binary: `mandate` CLI
  mandate-server/                      # binary: `mandate` daemon
  mandate-web/                         # binary: web UI server (P6)
  mandate-bots/                        # binary: telegram/signal bot adapter
/sdks/
  python/                                  # `mandate_client`
  typescript/                              # `@mandate/client`
/contracts/                                # Solidity (P8)
  AttestedValidator.sol
  AttestationRegistry.sol
  AuditAnchor.sol
  PolicyRegistry.sol
  SafeAttestedModule.sol
/zk-circuits/                              # P9
/tools/
  mock-x402-server/                        # binary
/schemas/                                  # JSON schemas (single source of truth)
  aprp_v1.json
  policy_v1.json
  x402_v1.json
  audit_event_v1.json
  decision_token_v1.json
  policy_receipt_v1.json
/migrations/
  V001__init.sql
  V002__policy.sql
  ...
/policies/reference/                       # P6: shipped baselines
/examples/
  docker-compose/
  base-sepolia-x402/
  marketplace-buyer/
  marketplace-seller/
/demo-scripts/
  red-team/
  sponsors/
/demo-agents/
  research-agent/                         # real-agent ETHPrague harness contract
/test-corpus/
  aprp/                                    # APRP samples (golden + adversarial)
  x402/                                    # x402 challenges
  policy/                                  # policies
/fuzz/
/docs/
  api/openapi.json                         # REST API contract
/packaging/
  deb/
  rpm/
/helm/
/appliance-image/
/.github/workflows/
  ci.yml
  release.yml
  reproducible-build.yml
```

---

## §9 Public API Stability Levels

| API | Stability |
|---|---|
| REST endpoints `/v1/*` | **stable** since 1.0 — semver |
| APRP JSON Schema v1 | **stable** — additions backward-compat |
| Policy YAML schema v1 | **stable** — additions backward-compat |
| Decision token format v1 | **stable** — internal but versioned |
| Audit event format v1 | **stable** — versioned per event |
| Rust crate APIs (`mandate-core`) | **unstable** until 1.0 — minor breaks ok |
| MCP tool surface | **stable** since 1.0 |
| Python/TS SDK | **stable** since 1.0 — semver |
| Solidity contracts | **immutable** post-deploy; new deploy = new addr |

---

## §10 Cross-Module Sequence Locks (parallel-impl rules)

To prevent race conditions when multiple agents implement different modules:

1. **Schema-first.** No module may invent a new wire format. All wire formats live in `/schemas/`. PR adding wire field must update schema first.
2. **Error code first.** No module may add new error variant without updating `§3.1` table.
3. **Migration first.** No module may CREATE TABLE outside `/migrations/`.
4. **No cross-crate `pub use` re-exports** unless declared here. (Prevents accidental coupling.)
5. **Telemetry metrics:** all metric names must match regex `^mandate_[a-z_]+(_total|_seconds|_bytes|_count|_status)$`. Document each in `mandate-core/src/telemetry/metrics.rs`.
6. **Audit event types:** can only be added if listed in `§5.4`. Adding a new type = update this file in same PR.
7. **Capability tokens:** every internal cross-zone call must carry a capability token (HMAC of action name + allowed-key-id-set + expiry). No bare RPC.

---

## §11 Reference test vectors

For implementers to self-verify their parsers/hashers match the spec.

### §11.1 APRP test vector A
Input file: `/test-corpus/aprp/golden_001_minimal.json`

```json
{
  "agent_id": "research-agent-01",
  "task_id": "demo-task-1",
  "intent": "purchase_api_call",
  "amount": { "value": "0.05", "currency": "USD" },
  "token": "USDC",
  "destination": {
    "type": "x402_endpoint",
    "url": "https://api.example.com/v1/inference",
    "method": "POST"
  },
  "payment_protocol": "x402",
  "chain": "base",
  "provider_url": "https://api.example.com",
  "x402_payload": null,
  "expiry": "2026-04-25T10:31:00Z",
  "nonce": "01HTAWX5K3R8YV9NQB7C6P2DGM",
  "expected_result": null,
  "risk_class": "low"
}
```

Expected canonical hash:
```
request_hash_sha256_hex = "<TO BE COMPUTED ON FIRST IMPLEMENTATION; LOCK BY GOLDEN TEST>"
```

> **Note for implementer:** Compute this hash with `serde_json_canonicalizer` then `sha2::Sha256` on first run. Save in `/test-corpus/aprp/golden_001_minimal.hash`. From then on, every other module must produce the same value.

### §11.2 Decision token test vector
Will be locked after first implementation per same procedure.

### §11.3 Audit event chain test vector
3-event chain in `/test-corpus/audit/chain_v1.jsonl` with expected `event_hash` and signature for each.

---

## §12 Forbidden patterns

These patterns **must not** appear in code. CI lint enforces.

1. **Float for money.** No `f32`/`f64` for `amount` or `cap_usd`. Use `BigDecimal`.
2. **`.unwrap()` outside test code.** Use `?` or explicit `.expect("invariant: ...")` with rationale.
3. **`std::env::var()` outside config loader.** All env access goes through one place.
4. **`println!` in library code.** Use `tracing::*` macros.
5. **Spawn tokio tasks without naming.** Use `tokio::task::Builder::new().name(...)`.
6. **Direct file access to key files outside `signing` module.** No leak.
7. **`tracing::info!(payload = ?req)` with raw request body.** Hash before logging.
8. **`Ok(_)` discard of error variants in audit writer.** Audit writes are critical-fail-fast.
9. **`SQLite BEGIN` without `IMMEDIATE`** when writing to budget or audit.
10. **Conditional compilation `#[cfg(test)]` for crypto correctness paths.** Tests must exercise real code.
