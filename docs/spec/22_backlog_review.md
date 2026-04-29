# Backlog Correctness Review (v1)

> **Účel:** Pred-implementačný review backlogu (`12_backlog.md`) s cross-checkmi voči `17_interface_contracts.md` a `19_knowledge_base.md`. Identifikuje konflikty, chybné gotchas, missing context, a critical safeguards. Critical fixes sú aplikované inline v backlogu; vyriešené issue sú v `§4 RESOLVED` sekcii.
>
> **Filozofia:** Lepšie chytiť bug v dokumentácii teraz, než ho debuggovať počas implementačného loopu pri 8 paralelných agentoch.

---

## §0 Resolution update 2026-04-26

Spec-hardening pass doplnil chýbajúce implementation artefakty:

- `/schemas/{aprp_v1,policy_v1,x402_v1,audit_event_v1,decision_token_v1}.json`
- `docs/api/openapi.json`
- `/test-corpus/{aprp,policy,x402,audit}/`
- `/demo-agents/research-agent/`
- `26_end_to_end_implementation_spec.md`

Tým sú čiastočne alebo úplne adresované ADV-06, ADV-07, ADV-09, SAFE-01, SAFE-02, SAFE-03 a SAFE-04. Zvyšné advisory položky ostávajú ako implementačné hardening notes pre konkrétne stories.

## §1 Critical conflicts (FIXED inline v 12_backlog.md)

### CONF-01 ✅ FIXED — `serde_jcs` is deprecated
- **Where:** E2-S1 gotcha hovorí "používať `serde_jcs` crate".
- **Conflict:** `19_knowledge_base.md §5.1` jasne hovorí: *"`serde_jcs` 0.2.x — effectively abandoned (last release ~2022). Use **`serde_json_canonicalizer`** (evik42, actively maintained, claims 100% RFC 8785 compliance)."*
- **Risk:** Implementer použije deprecated crate, dostane non-deterministic hashes na cross-language testoch.
- **Fix applied:** E2-S1 gotcha updated to `serde_json_canonicalizer`.

### CONF-02 ✅ FIXED — ULID nonce regex je incorrect
- **Where:** E2-S1 gotcha: `regex ^[0-9A-Z]{26}$ (ULID)`.
- **Conflict:** `17_interface_contracts.md §0` says: *"ULID (Crockford base32, 26 chars), regex `^[0-7][0-9A-HJKMNP-TV-Z]{25}$`."*
- **Risk:** Permissive regex accepts non-ULID strings (I, L, O, U → confusable with 1, 0). Will cause silent false positives in tests.
- **Fix applied:** E2-S1 + E2-S2 ULID regex updated to Crockford base32.

### CONF-03 ✅ FIXED — `BigDecimal` vs `rust_decimal`
- **Where:** E2-S1 gotcha "`BigDecimal` vs float pre `amount.value`"; E5-S1 gotcha "`BigDecimal` (Rust `bigdecimal` crate)".
- **Conflict:** `19_knowledge_base.md §5.6` says: *"`rust_decimal` (96-bit mantissa, 28 decimal places) — Recommended. Avoid `bigdecimal` unless you need precision >28 digits."*
- **Risk:** `bigdecimal` allocates on every op, slower for hot path (per-payment policy eval). Inconsistent crate choice across modules.
- **Fix applied:** All gotchas updated to `rust_decimal`.

### CONF-04 ✅ FIXED — `X-Payment` header is x402 v1
- **Where:** E16-S0 hovorí "po prijatí `X-Payment` header".
- **Conflict:** `19_knowledge_base.md §2.2` says: *"x402 v2 transport: `PAYMENT-REQUIRED`, `PAYMENT-SIGNATURE`, `PAYMENT-RESPONSE` (base64 JSON). Coinbase docs/SDK still also reference `X-PAYMENT` / `X-PAYMENT-RESPONSE` (legacy v1) — SDKs in the wild mix both casings. **Implement both, prefer v2.**"*
- **Risk:** Mock server only sends v1 headers; real x402 v2 providers won't work.
- **Fix applied:** E16-S0 acceptance updated to support BOTH header formats.

### CONF-05 ✅ FIXED — TPM PCR sealing wrong PCRs
- **Where:** E8-S3 acceptance "Key sealed na PCR 0–7 (boot integrity)".
- **Conflict:** `19_knowledge_base.md §6.4` says: *"PCR 7 = Secure Boot state, PCR 11 = unified kernel image, PCR 14 = MOK. **DON'T**: PCR 0 (firmware updates rebrick)."*
- **Risk:** Sealing to PCR 0 means BIOS update breaks LUKS unlock + key access. Production-fatal.
- **Fix applied:** E8-S3 acceptance updated to PCR 7+11+14.

### CONF-06 ✅ FIXED — Wrong gas cost estimate for on-chain DCAP
- **Where:** E16-S6 gotcha "DCAP verification on-chain je gas-heavy (~1M gas)".
- **Conflict:** `19_knowledge_base.md §1.3` says: *"Full on-chain verify of TDX quote: **~4M gas with RIP-7212 precompile, ~5M without**. ZK route (RISC Zero or SP1): **~250–400k gas** to verify SNARK + ~$0.05–0.20 in proof generation off-chain."*
- **Risk:** Implementer sets gas limit too low → tx OOG; or gives wrong cost estimate to sponsors.
- **Fix applied:** E16-S6 gotcha updated with correct ranges + ZK path.

### CONF-07 ✅ FIXED — Workspace crate list incomplete
- **Where:** E1-S1 acceptance "Rust workspace s `sbo3l-core`, `sbo3l-cli`, `mandate-sdk-rs`".
- **Conflict:** `17_interface_contracts.md §8` defines 11+ crates: `sbo3l-core`, `sbo3l-policy`, `sbo3l-storage`, `mandate-onchain`, `sbo3l-mcp`, `mandate-push`, `mandate-zk`, `sbo3l-cli`, `sbo3l-server`, `mandate-web`, `mandate-bots`. Plus SDKs in `/sdks/`.
- **Risk:** Foundation phase creates wrong workspace skeleton; later phases need to restructure.
- **Fix applied:** E1-S1 acceptance updated to "starting workspace per §8 with growth path documented."

---

## §2 Non-critical issues (advisory, NOT auto-applied)

These are real bugs but less load-bearing. Implementer should address but won't block.

### ADV-01 — E11-S1 dependency on E11-S2 across phases
- **Where:** E11-S1 in P6 says `blocked_by: E11-S2`, but E11-S2 is in P3.
- **Issue:** Looks circular but is just chronological — Web UI builds on top of CLI approval flow. Either:
  - Document explicitly that "E11-S2 (P3) provides the API; E11-S1 (P6) consumes it."
  - Or split E11-S2 into core API (P3) + CLI wrapper (P3).
- **Recommendation:** Add note in E11-S1 acceptance: *"E11-S2 must have completed in P3 to provide the approval API surface that this Web UI consumes."*

### ADV-02 — `sbo3l-core/src/chains/base.rs` belongs in separate crate
- **Where:** E16-S1, E16-S2 modules.
- **Issue:** Chain-specific code in core makes core depend on `alloy` heavyweight stack. Better: separate `mandate-onchain` crate (already in §8). Core trait → onchain implementation.
- **Recommendation:** Move chain modules to `/crates/sbo3l-onchain/src/chains/`.

### ADV-03 — Audit coverage tests cross-crate
- **Where:** E10-S5, E10-S6 in `/crates/sbo3l-core/tests/`.
- **Issue:** Coverage tests need to instantiate storage, policy, and emergency state — cross-crate. Better placed in workspace-level `/tests/integration/`.
- **Recommendation:** Move to `/tests/integration/audit_coverage.rs` with `#[test]` from E17-S2 suite.

### ADV-04 — Telegram bot security
- **Where:** E11-S4 "Bot posiela notification + pri reply (signed) prijíma approval."
- **Issue:** Telegram channel encryption is server-mediated; not E2E by default. Approval reply via Telegram weakens the signature trust model (man-in-middle scenario via Telegram server).
- **Recommendation:** Add gotcha: *"Telegram is notification-only; approval signing must happen out-of-band via signed payload from offline-protected admin key (not via plaintext bot reply). Bot reply triggers PWA / CLI flow, never directly approves."*

### ADV-05 — `tonic` migration to hyper 1.0 (v0.14)
- **Where:** Multiple stories use HTTP/gRPC; not currently called out.
- **Issue:** `19_knowledge_base.md §5.7` notes tonic 0.14 migrated to hyper 1.0; older docs/examples use tonic 0.10/0.11 (incompatible).
- **Recommendation:** Add CI check: pin `tonic >= 0.14`. Also re-check `axum` compatibility (axum 0.7+ for hyper 1.0).

### ADV-06 — Agent SDK transport for Unix socket
- **Where:** E2-S3 (Python SDK), E2-S4 (TS SDK).
- **Issue:** Unix socket from Python — needs `requests-unixsocket` or similar; from TS — needs custom HTTP agent. Acceptance doesn't specify.
- **Recommendation:** Add to acceptance: *"Python SDK supports both Unix socket transport (via `requests-unixsocket` or `httpx` with custom transport) and TCP. TS SDK same with `node:net` socket adapter or custom `Agent` for `undici`."*

### ADV-07 — Decision token TTL not specified
- **Where:** E8-S5 acceptance.
- **Issue:** `17_interface_contracts.md §4.4` requires `expires_at > now()` check, but story doesn't say what default TTL the policy emits.
- **Recommendation:** Add to acceptance: *"Decision token TTL default 60s, max 600s (configurable). Signer rejects token older than configured max."*

### ADV-08 — RPC URL allowlist in IPAddressAllow
- **Where:** Implicit in `20_linux_server_install.md §6` systemd unit; not enforced in code.
- **Issue:** systemd `IPAddressAllow=` uses literal IP, not hostname. RPC providers rotate IPs — config drift.
- **Recommendation:** Add story or note to E16-S1: *"RPC connection check: log when DNS resolves to non-allowlisted IP; fall back to hostname allowlist via DNS-over-HTTPS for known providers."*

### ADV-09 — Test corpus path inconsistency
- **Where:** E2-S1 test corpus implied; E6-S1 says `/test-corpus/x402/`.
- **Issue:** Backlog doesn't define corpus directory layout.
- **Recommendation:** Document in `17_interface_contracts.md §8`: `/test-corpus/{aprp,x402,policy,decision_tokens,audit}/`.

### ADV-10 — Missing story: Vault CA private key rotation
- **Where:** E3-S2 mentions vault holds private CA, but no story for CA rotation.
- **Issue:** CA rotation is real ops need (annual, after compromise, etc.). Not a P1 concern but worth a story.
- **Recommendation:** Add E3-S5 (P2): "CA rotation flow — generate new CA, sign trust-bridge cross-cert, invalidate old CA after grace period."

---

## §3 Missing safeguards (RECOMMENDED additions)

These are *defenses* the backlog doesn't yet specify. Adding them would prevent likely implementation bugs.

### SAFE-01 — Forbid `unwrap()` outside test code
- **Where:** Should be CI lint.
- **Recommendation:** Add to E1-S1 CI: `clippy::unwrap_used` warn level + manual review for any `expect()` with rationale. Already in `17_interface_contracts.md §10` but not enforced.

### SAFE-02 — Mandatory `#[deny_unknown_fields]` audit
- **Where:** All serde struct definitions for wire types.
- **Recommendation:** Add lint: search for `#[derive(Deserialize)]` without `#[serde(deny_unknown_fields)]` on wire types. Block PR if violated.

### SAFE-03 — Migration replay test
- **Where:** E1-S1 + storage stories.
- **Recommendation:** CI test that runs all migrations forward, checks DB state, verifies no schema drift between fresh-migrate vs incremental-migrate.

### SAFE-04 — Decimal precision boundary tests
- **Where:** E5 (Budget) + E16 (chain integration).
- **Recommendation:** Test budget operations with edge decimals: `0.000001` (USDC dust), `999999999999.99` (overflow approach), negative (must reject).

### SAFE-05 — TLS cert pin rotation flow
- **Where:** E6-S2 cert pin not rotation-aware.
- **Recommendation:** Add to E6-S2: *"Support backup cert pin (`cert_pin_sha256_backup`) for rotation; accept either; warn on use of backup; admin must update primary within 30 days."*

### SAFE-06 — Hardware kill switch debouncing
- **Where:** E12-S3 mentions double-press but not debounce time.
- **Recommendation:** Acceptance: *"Hardware kill switch double-press window 100-2000ms (configurable). Below 100ms → debounce noise. Above 2000ms → considered separate events."*

### SAFE-07 — TEE attestation freshness max age
- **Where:** E9-S5 attestation drift detection.
- **Recommendation:** Add: *"Attestation evidence used for signing must be ≤ 60s old (configurable). Older evidence → re-attest before sign."*

### SAFE-08 — DB transaction isolation level
- **Where:** SQLite always serializable, but `BEGIN IMMEDIATE` vs `BEGIN DEFERRED` matters for budget reservations.
- **Recommendation:** All write transactions must `BEGIN IMMEDIATE` (prevents read-then-write races). Add to lint.

### SAFE-09 — Audit log GC retention
- **Where:** Audit retention not specified in backlog (only in 10_data_model.md §J.3).
- **Recommendation:** Add to E10-S1: *"Configurable retention default 7 years (compliance). GC task removes events older than retention WHILE preserving Merkle root chain integrity."*

### SAFE-10 — Test agent identity for SDK examples
- **Where:** E2-S3, E2-S4 example agents.
- **Recommendation:** Add: *"SDK examples ship with `dev-agent-key.pem` test cert + clear `// DEV ONLY — do not use in production` warning."*

### SAFE-11 — Reproducible build C dep pinning
- **Where:** E14-S5 reproducible build verification.
- **Recommendation:** Add: *"All C deps (libtss2, libsecp256k1, libsystemd if linked) pinned via container digest. CI test: 2× build in different container instantiations of same digest → byte-match."*

### SAFE-12 — Signer attestation token replay protection
- **Where:** E8-S5 acceptance.
- **Recommendation:** Add: *"Decision token nonce stored in memory for token TTL; signer rejects re-use within TTL window."*

### SAFE-13 — Multi-RPC quorum ordering attack
- **Where:** E7-S3 quorum logic.
- **Recommendation:** Add gotcha: *"RPC quorum must be over **state at same block number**. Different RPCs at different blocks = false disagreement. Pin block, query all, compare."*

### SAFE-14 — Smart contract deployment determinism
- **Where:** E16-S3, E16-S6 contract deploys.
- **Recommendation:** Add: *"All contracts deployed via `CREATE2` with deterministic salt (e.g. `keccak256(release_version)`) so addresses are predictable across testnet/mainnet."*

### SAFE-15 — On-chain anchor tx batching
- **Where:** E16-S7 audit anchor cost <$0.01.
- **Recommendation:** Add: *"Batch up to 30 daily roots in single anchor tx if multi-chain anchoring (saves gas, predictable cadence)."*

---

## §4 Sanity-checks PASSED ✅

These checks were performed and confirmed correct:

- ✅ APRP nonce field is in canonical hash input (replay protection works).
- ✅ Audit hash chain genesis = 64 zeros (matches `17 §5.2`).
- ✅ Decision token uses Ed25519 (not ECDSA — separate from transaction signing key).
- ✅ Policy DSL uses `regorus` (not OPA Wasm — TCB minimization).
- ✅ k256 chosen over secp256k1 (audit pedigree, reproducibility).
- ✅ Trust boundaries: HSM never sees decision context, policy engine never sees private key.
- ✅ All stories reference acceptance demos (every story has `accept:` field).
- ✅ Phase exit criteria documented.
- ✅ Story dependencies form a DAG (no circular).
- ✅ Workspace permissions match systemd user (`mandate`).
- ✅ Production lint blocks all dev-mode features.
- ✅ Emergency state singleton pattern is correct (id=1 row).
- ✅ x402 nonce is bytes32 random (not sequential — matches EIP-3009 requirements).
- ✅ Smart account session keys revocable on-chain in single tx.

---

## §5 Coverage gaps (DOCUMENTED, not blocked)

Things the backlog **acknowledges as out-of-scope** but worth documenting for future:

- 🟡 No multi-region active-active deployment (single-tenant assumption).
- 🟡 No automatic key compromise recovery (manual via runbook E12-S5).
- 🟡 No formal verification of policy engine (informal lint + test only).
- 🟡 No HSM firmware attestation (relying on vendor trust).
- 🟡 Limited support for non-EVM chains (Solana mentioned in §2.4 but not impl).
- 🟡 No real-time streaming payment (Sablier/Superfluid noted in 08_data_flow §H.4 Scenario C).
- 🟡 No per-tenant policy isolation (single admin set per vault — multi-tenant in OQ-08).

---

## §6 Cross-document consistency matrix

| Topic | 12_backlog | 17_contracts | 19_KB | Status |
|---|---|---|---|---|
| ULID regex | (was `^[0-9A-Z]{26}$`) → fixed | `^[0-7][0-9A-HJKMNP-TV-Z]{25}$` | (not specified) | ✅ FIXED |
| JSON canonicalization | (was `serde_jcs`) → fixed | (not specified) | `serde_json_canonicalizer` | ✅ FIXED |
| Decimal type | (was `bigdecimal`) → fixed | (not specified) | `rust_decimal` | ✅ FIXED |
| Workspace crates | (was 3) → fixed | 11+ in §8 | (not specified) | ✅ FIXED |
| TPM PCRs | (was 0-7) → fixed | (not specified) | 7+11+14 in §6.4 | ✅ FIXED |
| x402 headers | (was X-Payment) → fixed | (not specified) | PAYMENT-* in §2.2 | ✅ FIXED |
| Gas cost (DCAP) | (was ~1M) → fixed | (not specified) | 4-5M / ~250-400k ZK in §1.3 | ✅ FIXED |
| Error codes | references §3 | 70+ codes in §3.1 | — | ✅ consistent |
| Audit event types | references §5 | 40+ types in §5.4 | — | ✅ consistent |
| Storage layout | references §8 | full layout in §7 | — | ✅ consistent |
| TEE attestation libs | (not detailed) | — | dcap-qvl, virtee/sev in §1.9 | 🟡 add to E9 stories |
| HSM PKCS#11 paths | generic | — | Nitrokey/YubiHSM specifics in §5.2 | 🟡 add to E8-S2 |

---

## §7 Recommended next actions

1. **Apply CRITICAL fixes inline** (§1) — DONE in this PR.
2. **Adopt advisory recommendations** (§2) — implementer should action during phases.
3. **Add safeguard stories** (§3) — these become new sub-stories during phase impl.
4. **Track coverage gaps** (§5) — open issues in `14_open_questions.md`.
5. **Re-run review at start of each phase** to catch new conflicts as code lands.

---

## §8 Self-test for review correctness

This review itself should be reviewed. Sanity checks:

- ✅ Every CRITICAL fix references both source documents (backlog + contracts/KB).
- ✅ Every advisory has a recommendation (not just a complaint).
- ✅ Every safeguard is *additive* (doesn't conflict with existing).
- ✅ All RESOLVED issues have explicit fix locations.
- ✅ No new architectural decisions made here (review, not design).
