# Agent Assignment Matrix

> **Účel:** Tento súbor mapuje stories z `12_backlog.md` na konkrétne agentové sloty pre paralelnú implementáciu. Každý slot je samostatne implementovateľný (žiadne hidden dependencies); slot vie čo má urobiť, ktoré súbory ovplyvní, čo musí passnúť pred merge a aké inputy dostane.
>
> **Loop spawn pattern (orchestrator → agent):**
> ```
> Agent(
>   description: "<phase> <slot-name>",
>   subagent_type: "general-purpose" | "Explore",
>   prompt: <self-contained brief s linkami na contracts, KB, demos, story IDs>
> )
> ```
>
> **Pravidlo:** Žiadny agent nesmie modifikovať *súbory mimo svojich `modules`*. Cross-cutting changes idú cez **integration slot** (kapitola §13).

---

## §0 Agent kompetencie / typy

| Subagent type | Vhodný pre |
|---|---|
| **general-purpose** | Väčšina implementačných slotov: písanie Rust crate, schémy, contractov, testov |
| **Explore** | Iba research / discovery; *nie* na write tasks |
| **claude-code-guide** | Iba ak agent potrebuje pomoc s Claude Code samotným |

V tomto projekte takmer všetko = `general-purpose`. Pri implementácii, ktorá vyžaduje cross-checks na hotový code, doplníme `Explore` slot ako pre-flight.

---

## §1 Slot conventions

Každý slot má:

```
SLOT-PX-NN: <human-name>
  Phase: P<N>
  Stories covered: <list of E-S IDs>
  modules: <list of file paths from §17 §8>
  blocked_by: <slot IDs from prior phases>
  parallel_with: <slot IDs in same phase>
  estimated_size: S | M | L | XL  (relative effort)
  agent_brief: <what to put in subagent prompt>
  pre_read: <files agent must read first>
  post_demo: <demo IDs that must pass>
  hand_back: <what to report to orchestrator>
```

---

# Phase 0 — 5 paralelných slotov

## SLOT-P0-01: Repo bootstrap & CI
- **Stories:** E1-S1
- **Modules:** `/Cargo.toml`, `/.github/workflows/ci.yml`, `/rust-toolchain.toml`, `/.gitignore`
- **blocked_by:** —
- **parallel_with:** SLOT-P0-02, P0-03, P0-04, P0-05
- **size:** S
- **brief:** "Set up Rust workspace per `17_interface_contracts.md §8`. Pin toolchain `1.84.0`. CI green for `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `cargo audit`. Reproducible build cache."
- **pre_read:** `17_interface_contracts.md §0, §8`, `19_knowledge_base.md §5.11`
- **post_demo:** `D-P0-01`
- **hand_back:** "Workspace ready, CI green on empty crates."

## SLOT-P0-02: Licensing & governance
- **Stories:** E1-S2
- **Modules:** `/LICENSE`, `/CONTRIBUTING.md`, `/SECURITY.md`, `/CODE_OF_CONDUCT.md`
- **blocked_by:** —
- **parallel_with:** SLOT-P0-01, P0-03, P0-04, P0-05
- **size:** S
- **brief:** "Apache 2.0 LICENSE. SECURITY.md with security@<domain> + GPG fingerprint placeholder + 90-day disclosure. CONTRIBUTING.md requires DCO/GPG signed commits."
- **post_demo:** `D-P0-02`

## SLOT-P0-03: Telemetry scaffold
- **Stories:** E1-S3
- **Modules:** `/crates/mandate-core/src/telemetry/mod.rs`, `/crates/mandate-core/src/telemetry/metrics.rs`, `Cargo.toml` deps
- **blocked_by:** SLOT-P0-01 (workspace)
- **parallel_with:** P0-04, P0-05 (only after P0-01)
- **size:** M
- **brief:** "Implement telemetry per `19_knowledge_base.md §5.8`: tracing + tracing-subscriber + opentelemetry stack. JSON logs by default; OTLP export configurable. **Must NOT log payment payload bytes or key material** — assertion in tests."
- **pre_read:** `17_interface_contracts.md §10` (forbidden patterns), `19_knowledge_base.md §5.8`
- **post_demo:** `D-P0-03`

## SLOT-P0-04: Error catalog
- **Stories:** E1-S4
- **Modules:** `/crates/mandate-core/src/error.rs`
- **blocked_by:** SLOT-P0-01
- **parallel_with:** P0-03, P0-05
- **size:** S
- **brief:** "Implement top-level `Error` enum + sub-enums. RFC 7807 problem+json mapping. Every variant has `code` (str), `http_status` (u16), `audit_severity`. Match exactly the table in `17_interface_contracts.md §3.1` — copy-paste codes."
- **pre_read:** `17_interface_contracts.md §3` (full)
- **post_demo:** `D-P0-04`

## SLOT-P0-05: Configuration model
- **Stories:** E1-S5
- **Modules:** `/crates/mandate-core/src/config/mod.rs`, `/examples/mandate.toml`
- **blocked_by:** SLOT-P0-01
- **parallel_with:** P0-03, P0-04
- **size:** S
- **brief:** "Implement TOML config per `17_interface_contracts.md §1`. Env override `MANDATE__SECTION__KEY`. Production lint asserts (no dev_key, no 0.0.0.0 listen, attestation_required on operational)."
- **pre_read:** `17_interface_contracts.md §1, §10`
- **post_demo:** `D-P0-05`

---

# Phase 1 — 4 paralelné skupiny (4 sloty)

## SLOT-P1-A: APRP + SDK
- **Stories:** E2-S1, E2-S2, E2-S3, E2-S4
- **Modules:** `/schemas/aprp_v1.json`, `/test-corpus/aprp/`, `/crates/mandate-core/src/protocol/{aprp,validator}.rs`, `/sdks/python/`, `/sdks/typescript/`
- **blocked_by:** SLOT-P0-01, P0-04, P0-05
- **parallel_with:** SLOT-P1-B, P1-C, P1-D
- **size:** L
- **brief:** "Implement APRP per `17_interface_contracts.md §2`. JSON schema → Rust types (use `typify` or manual). Strict validator with golden + adversarial corpus tests. Python + TS SDKs auto-generated from schema. **JCS canonical hashing via `serde_json_canonicalizer`** (NOT `serde_jcs`)."
- **pre_read:** `17_interface_contracts.md §2`, `19_knowledge_base.md §5.1` (canonical JSON section)
- **post_demo:** `D-P1-01`, `D-P1-02`, `D-P1-03`, `D-P1-04`, `D-P1-05`

## SLOT-P1-B: Gateway + Auth
- **Stories:** E3-S1, E3-S2, E3-S4
- **Modules:** `/crates/mandate-core/src/server/{transport,auth,rate_limit}.rs`, `/crates/mandate-cli/src/admin/agent.rs`
- **blocked_by:** SLOT-P0-01, P0-04, P0-05
- **parallel_with:** SLOT-P1-A, P1-C, P1-D
- **size:** L
- **brief:** "Implement REST + gRPC server with Unix socket (default) and TCP loopback. mTLS for agents (vault as CA). Rate limit per agent. **No bind to 0.0.0.0 ever** — assertion in config validator (already in P0-05)."
- **pre_read:** `17_interface_contracts.md §1, §7`, `19_knowledge_base.md §5.7, §5.12`
- **post_demo:** `D-P1-06`, `D-P1-07`, `D-P1-08`

## SLOT-P1-C: Signing
- **Stories:** E8-S1, E8-S5
- **Modules:** `/crates/mandate-core/src/signing/{mod,backend/local_dev,backend/encrypted_file,decision_token}.rs`
- **blocked_by:** SLOT-P0-01, P0-04
- **parallel_with:** SLOT-P1-A, P1-B, P1-D
- **size:** L
- **brief:** "Implement `SigningBackend` trait. Two backends: `local_dev_key` (ephemeral, dev only) + `encrypted_file` (age-encrypted, passphrase via systemd-creds). **Signer rejects payload without valid decision token** (HMAC or Ed25519). Use `k256` for secp256k1, `age` for encryption, `secrecy` + `region::lock` for key buffers."
- **pre_read:** `17_interface_contracts.md §4`, `19_knowledge_base.md §5.1, §5.10`
- **post_demo:** `D-P1-09`, `D-P1-10`, `D-P1-11`, `D-P1-12`

## SLOT-P1-D: Mock x402 server
- **Stories:** E16-S0
- **Modules:** `/tools/mock-x402-server/`
- **blocked_by:** SLOT-P0-01
- **parallel_with:** SLOT-P1-A, P1-B, P1-C
- **size:** M
- **brief:** "Standalone Rust binary that emits valid x402 v2 challenges (transport v2 headers per `19_knowledge_base.md §2.2`). Configurable port + self-signed TLS. Endpoints `/api/inference`, `/api/dataset`, `/api/compute-job`. After valid `PAYMENT-SIGNATURE`, return 200 + JSON. Used as primary x402 source for demos to avoid external dependency."
- **pre_read:** `19_knowledge_base.md §2`
- **post_demo:** `D-P1-13`

---

# Phase 2 — 3 paralelné sloty

## SLOT-P2-A: Policy engine
- **Stories:** E4-S1, E4-S2, E4-S3, E4-S4, E4-S5
- **Modules:** `/crates/mandate-policy/`, `/migrations/V002__policy.sql`, `/schemas/policy_v1.json`, `/crates/mandate-cli/src/{lint,dry_run}.rs`
- **blocked_by:** SLOT-P1-A (APRP types needed)
- **parallel_with:** SLOT-P2-B, P2-C
- **size:** XL
- **brief:** "Build policy engine per `11_policy_model.md` and `19_knowledge_base.md §5.3`. Use `regorus` for embedded Rego eval. YAML→Rego compiler. Versioned policy storage (immutable, hash-chained). M-of-N admin signature verification. Lint CLI (10+ rules per `11_policy_model.md §K.6`). Dry-run command. **All eval P99 < 50ms on 30-rule policy.**"
- **pre_read:** `11_policy_model.md`, `17_interface_contracts.md §10` (forbidden patterns), `19_knowledge_base.md §5.3`
- **post_demo:** `D-P2-01..08`

## SLOT-P2-B: Budget ledger
- **Stories:** E5-S1, E5-S2, E5-S3
- **Modules:** `/crates/mandate-storage/src/budget.rs`, `/migrations/V003__budget.sql`, `/crates/mandate-core/src/scheduler/budget_reset.rs`
- **blocked_by:** SLOT-P0-01, P0-04
- **parallel_with:** SLOT-P2-A, P2-C
- **size:** L
- **brief:** "SQLite budget tables per `10_data_model.md §J.1.6-7`. Atomic reserve/commit/release with `BEGIN IMMEDIATE`. Multi-scope (daily/weekly/monthly/per-provider/per-token/per-task). Periodic reset task. Concurrent-safe (chaos test 100 parallel reserves). Use `rust_decimal` for USD."
- **pre_read:** `19_knowledge_base.md §5.5, §5.6` (storage + decimal sections)
- **post_demo:** `D-P2-09..13`

## SLOT-P2-C: x402 verifier (mock-only at P2)
- **Stories:** E6-S1, E6-S2, E6-S3, E6-S4, E2-S2-EXT
- **Modules:** `/crates/mandate-core/src/x402/`, `/test-corpus/x402/`, `/crates/mandate-storage/src/nonce_store.rs`
- **blocked_by:** SLOT-P0-01
- **parallel_with:** SLOT-P2-A, P2-B
- **size:** L
- **brief:** "x402 v2 challenge parser (per `19_knowledge_base.md §2`). Cert pin (leaf SPKI hash). Amount/asset/network consistency. Provider reputation rolling score. Persistent nonce store (replace P1 in-memory bloom)."
- **pre_read:** `19_knowledge_base.md §2`
- **post_demo:** `D-P2-14..17`, `D-P2-18`, `D-P2-RT-01`

---

# Phase 3 — 3 paralelné sloty

## SLOT-P3-A: Audit log
- **Stories:** E10-S1, E10-S2, E10-S3, E10-S5, E10-S6
- **Modules:** `/crates/mandate-storage/src/audit.rs`, `/crates/mandate-core/src/audit/`, `/migrations/V004__audit.sql`, `/crates/mandate-cli/src/audit.rs`
- **blocked_by:** SLOT-P0-04
- **parallel_with:** SLOT-P3-B, P3-C
- **size:** L
- **brief:** "Hash-chained audit log per `17_interface_contracts.md §5`. SQLite tabuľka with INSERT-only triggers. Daily Merkle root + signed manifest. S3-compatible export. Coverage assertion test (every mutating action emits event). No-PII assertion test."
- **pre_read:** `17_interface_contracts.md §5`
- **post_demo:** `D-P3-01..04`, `D-P3-13..14`

## SLOT-P3-B: Emergency controls
- **Stories:** E12-S1, E12-S2, E12-S3, E12-S4, E12-S5
- **Modules:** `/crates/mandate-core/src/emergency/`
- **blocked_by:** SLOT-P3-A (needs audit)
- **parallel_with:** SLOT-P3-A (after audit ready), P3-C
- **size:** L
- **brief:** "Singleton EmergencyState. freeze_all / resume (M-of-N). Pause agent / revoke provider / blacklist recipient. Hardware kill switch via evdev (configurable device path, double-press 1s window). Anomaly auto-freeze (rule-based, fp rate < 5%). Recovery procedure (multisig + 24h delay). **Freeze effect must be < 100ms.**"
- **post_demo:** `D-P3-05..11`

## SLOT-P3-C: Approval CLI
- **Stories:** E11-S2
- **Modules:** `/crates/mandate-cli/src/approvals.rs`
- **blocked_by:** SLOT-P3-A
- **parallel_with:** SLOT-P3-A, P3-B
- **size:** M
- **brief:** "CLI: `mandate approvals pending` lists pending. `mandate approvals sign <id>` admin signs (file key for now, HSM in P5). TTL countdown. Signed payload sent to vault via API."
- **post_demo:** `D-P3-12`

---

# Phase 4 — 3 paralelné sloty

## SLOT-P4-A: Transaction simulator
- **Stories:** E7-S1, E7-S2, E7-S3, E10-S7
- **Modules:** `/crates/mandate-core/src/simulator/`, `/crates/mandate-core/src/chains/rpc_health.rs`
- **blocked_by:** SLOT-P0-05, KB §5.4
- **parallel_with:** SLOT-P4-B, P4-C
- **size:** L
- **brief:** "Simulator using `alloy` (per `19_knowledge_base.md §5.4`). `eth_call` + `debug_traceCall` (fallback if not available). State pinning. Method selector whitelist per policy. Multi-RPC quorum (default 2-of-3). RPC health check + quarantine."
- **pre_read:** `19_knowledge_base.md §5.4`
- **post_demo:** `D-P4-01..04`, `D-P4-07`, `D-P4-RT-02`

## SLOT-P4-B: Chain integration (Base/Polygon/Arbitrum)
- **Stories:** E16-S1, E16-S2
- **Modules:** `/crates/mandate-core/src/chains/{base,polygon,arbitrum}.rs`, `/examples/base-sepolia-x402/`
- **blocked_by:** SLOT-P4-A (simulator), P2-C (x402 verifier), P1-C (signing)
- **parallel_with:** SLOT-P4-A, P4-C
- **size:** L
- **brief:** "Chain configs per `19_knowledge_base.md §2.4`. Base Sepolia first (chain_id 84532, USDC). Live x402 payment end-to-end. Polygon + Arbitrum parity."
- **pre_read:** `19_knowledge_base.md §2`
- **post_demo:** `D-P4-05`, `D-P4-06`, `D-P4-10`, `D-P4-11`

## SLOT-P4-C: Settlement watcher + replay protection
- **Stories:** E2-S5, E2-S6, E10-S8
- **Modules:** `/crates/mandate-storage/src/settlement.rs`, `/crates/mandate-core/src/server/idempotency.rs`, `/crates/mandate-core/src/protocol/replay.rs`
- **blocked_by:** SLOT-P4-A, P3-A
- **parallel_with:** SLOT-P4-A, P4-B
- **size:** M
- **brief:** "Settlement watcher: monitor tx confirmation depth, audit `settlement_complete`/`settlement_failed`, release reservation on failure. `Idempotency-Key` header (RFC draft). Cross-restart nonce persistence test."
- **post_demo:** `D-P4-08`, `D-P4-09`, `D-P4-RT-03`

---

# Phase 5 — 4 paralelné sloty

## SLOT-P5-A: PKCS#11 backend
- **Stories:** E8-S2, E8-S6, E8-S7
- **Modules:** `/crates/mandate-core/src/signing/backend/pkcs11.rs`, `/crates/mandate-core/src/signing/health.rs`, `/crates/mandate-core/src/config/production_lint.rs`
- **blocked_by:** SLOT-P1-C
- **parallel_with:** SLOT-P5-B, P5-C, P5-D
- **size:** L
- **brief:** "PKCS#11 backend via `cryptoki` (per `19_knowledge_base.md §5.2`). Test on YubiHSM 2 + Nitrokey HSM 2 + SoftHSM (CI). Backend health check (status `healthy`/`degraded`/`offline`). Production lint blocks dev backends in production mode."
- **pre_read:** `19_knowledge_base.md §5.2`
- **post_demo:** `D-P5-01`, `D-P5-02`, `D-P5-05`, `D-P5-06`

## SLOT-P5-B: TPM 2.0 backend
- **Stories:** E8-S3
- **Modules:** `/crates/mandate-core/src/signing/backend/tpm.rs`
- **blocked_by:** SLOT-P1-C
- **parallel_with:** SLOT-P5-A, P5-C, P5-D
- **size:** L
- **brief:** "TPM 2.0 via `tss-esapi`. Key sealed to PCR 7 (Secure Boot state) + 11 (UKI). **NOT PCR 0/2** (firmware update breaks). Negative test: ukradnutý disk → key sa neodomkne."
- **pre_read:** `19_knowledge_base.md §5.2, §6.4`
- **post_demo:** `D-P5-03`, `D-P5-04`

## SLOT-P5-C: Admin enrollment + multisig
- **Stories:** E13-S1, E13-S2
- **Modules:** `/crates/mandate-cli/src/admin/enroll.rs`, `/crates/mandate-core/src/governance/`
- **blocked_by:** SLOT-P2-A
- **parallel_with:** SLOT-P5-A, P5-B, P5-D
- **size:** M
- **brief:** "Bootstrap admin during init. Subsequent admins added via M-of-N existing admins. Setup wizard `mandate init --interactive`. Signature aggregation for policy mutations."
- **post_demo:** `D-P5-07`, `D-P5-08`

## SLOT-P5-D: MCP server
- **Stories:** E16-S5
- **Modules:** `/crates/mandate-mcp/`
- **blocked_by:** SLOT-P1-A, P1-B
- **parallel_with:** SLOT-P5-A, P5-B, P5-C
- **size:** M
- **brief:** "MCP server exposing tools: `payment.request`, `payment.simulate`, `payment.status`, `attestation.get`, `audit.tail`. Auth via vault-issued credential. Per `19_knowledge_base.md §8`."
- **pre_read:** `19_knowledge_base.md §8`
- **post_demo:** `D-P5-09`, `D-P5-10`

---

# Phase 6 — 4 paralelné sloty

## SLOT-P6-A: Web UI + WebAuthn
- **Stories:** E11-S1, E11-S5, E11-S6, E11-S7
- **Modules:** `/web-ui/`, `/crates/mandate-web/`
- **blocked_by:** SLOT-P3-C
- **parallel_with:** SLOT-P6-B, P6-C, P6-D
- **size:** XL
- **brief:** "SvelteKit (preferred) or SolidJS UI on loopback HTTPS. Pages: dashboard, agents, policies, approvals, budgets, audit, emergency. WebAuthn approval signing. **No third-party CDN; offline assets only.** TTL enforcement, signature verification, multi-approval aggregation."
- **post_demo:** `D-P6-01`, `D-P6-02`, `D-P6-12`, `D-P6-13`, `D-P6-14`

## SLOT-P6-B: Push notification + bots
- **Stories:** E11-S3, E11-S4
- **Modules:** `/crates/mandate-push/`, `/relay-server/`, `/crates/mandate-bots/`
- **blocked_by:** SLOT-P3-C
- **parallel_with:** SLOT-P6-A, P6-C, P6-D
- **size:** M
- **brief:** "Self-hosted push relay (ntfy.sh-compatible). Signed webhook payload. Telegram bot opt-in (default disabled)."
- **post_demo:** `D-P6-03`, `D-P6-04`

## SLOT-P6-C: RBAC + webhooks + digest
- **Stories:** E13-S3, E10-S9, E10-S10
- **Modules:** `/crates/mandate-core/src/governance/rbac.rs`, `/crates/mandate-core/src/webhooks/`, `/crates/mandate-core/src/audit/digest.rs`
- **blocked_by:** SLOT-P5-C
- **parallel_with:** SLOT-P6-A, P6-B, P6-D
- **size:** M
- **brief:** "Role table + JWT roles claim. Webhook subscriptions (signed POST + retry + DLQ). Weekly digest email via local SMTP."
- **post_demo:** `D-P6-05`, `D-P6-06`, `D-P6-07`

## SLOT-P6-D: Documentation
- **Stories:** E15-S1, E15-S2, E15-S3, E15-S4
- **Modules:** `/docs/`, `/policies/reference/`, `/configs/apparmor/`
- **blocked_by:** SLOT-P5-A (HSM stable for hardening guide)
- **parallel_with:** SLOT-P6-A, P6-B, P6-C
- **size:** M
- **brief:** "Quickstart, threat model summary, policy authoring guide. 5 reference policies (lint clean). LangChain/AutoGen/MCP cookbooks. Hardening guide (AppArmor/SELinux/systemd directives per `19_knowledge_base.md §6`)."
- **pre_read:** `19_knowledge_base.md §6`
- **post_demo:** `D-P6-08..11`

---

# Phase 7 — 4 paralelné sloty

## SLOT-P7-A: Self-signed attestation + drift
- **Stories:** E9-S1, E9-S4, E9-S5
- **Modules:** `/crates/mandate-core/src/attestation/{self_signed,drift,audit_link}.rs`
- **blocked_by:** SLOT-P3-A
- **parallel_with:** SLOT-P7-B, P7-C, P7-D
- **size:** M
- **brief:** "Self-signed attestation as baseline (when no TEE). Composite measurement = `H(binary_sha256 || policy_hash || config_hash)`. External verifier CLI. Drift detection (periodic re-attest, alert + auto-freeze on change). Audit event `attestation_ref` field."
- **post_demo:** `D-P7-01`, `D-P7-05`, `D-P7-06`

## SLOT-P7-B: TDX attestation
- **Stories:** E9-S2, E8-S4
- **Modules:** `/crates/mandate-core/src/attestation/tdx.rs`, `/crates/mandate-core/src/signing/backend/tee_sealed.rs`
- **blocked_by:** SLOT-P7-A
- **parallel_with:** SLOT-P7-C, P7-D
- **size:** XL
- **brief:** "Intel TDX quote generation via `dcap-qvl` (per `19_knowledge_base.md §1.1, §1.9`). Use **configfs-tsm path** (kernel ≥ 6.7), NOT vsock. PCCS configurable. Verifier with Intel root certs. TEE-sealed key (KMS-as-TApp pattern from dstack reference)."
- **pre_read:** `19_knowledge_base.md §1.1, §1.3, §1.7, §1.8`
- **post_demo:** `D-P7-02`, `D-P7-03`, `D-P7-07`

## SLOT-P7-C: SEV-SNP attestation
- **Stories:** E9-S3
- **Modules:** `/crates/mandate-core/src/attestation/sev_snp.rs`
- **blocked_by:** SLOT-P7-A
- **parallel_with:** SLOT-P7-B, P7-D
- **size:** L
- **brief:** "AMD SEV-SNP via `virtee/sev` with `crypto_nossl` feature. VCEK fetch + cache (KDS). Integration test on EPYC or Hetzner SEV-SNP."
- **pre_read:** `19_knowledge_base.md §1.2`
- **post_demo:** `D-P7-04`

## SLOT-P7-D: Packaging + reproducible builds
- **Stories:** E14-S1, E14-S2, E14-S3, E14-S5, E10-S11
- **Modules:** `/.github/workflows/release.yml`, `/.github/workflows/reproducible-build.yml`, `/packaging/{deb,rpm}/`, `/examples/docker-compose/`, `/crates/mandate-core/src/incident/`
- **blocked_by:** SLOT-P0-01
- **parallel_with:** SLOT-P7-A, B, C
- **size:** L
- **brief:** "musl static binary x86_64 + ARM64. Cosign v3 signed (`--bundle`). SLSA L3 provenance via `actions/attest-build-provenance`. `.deb` + `.rpm` with hardened systemd unit (per `19_knowledge_base.md §6.1`). Docker compose example. Reproducible-build verification CI. Forensic incident bundle export."
- **pre_read:** `19_knowledge_base.md §5.11, §6`
- **post_demo:** `D-P7-08..12`

---

# Phase 8 — 5 paralelných slotov (HACKATHON HERO PHASE)

## SLOT-P8-A: Safe attested module
- **Stories:** E16-S3
- **Modules:** `/contracts/SafeAttestedModule.sol`, `/crates/mandate-onchain/src/safe.rs`
- **blocked_by:** SLOT-P7-A or B
- **parallel_with:** SLOT-P8-B, C, D, E
- **size:** L
- **brief:** "Safe modul (per `19_knowledge_base.md §4.8`) accepting user op only with attestation. Vault signs session key auth. Testnet demo (Base Sepolia). Use `rhinestonewtf/safe7579` adapter."
- **pre_read:** `19_knowledge_base.md §4.4, §4.5, §4.8`
- **post_demo:** `D-P8-01`, `D-P8-02`

## SLOT-P8-B: Custom ERC-4337 attested validator (HERO)
- **Stories:** E16-S6
- **Modules:** `/contracts/AttestedValidator.sol`, `/contracts/AttestationRegistry.sol`
- **blocked_by:** SLOT-P7-B (TDX) or SLOT-P7-C (SEV)
- **parallel_with:** SLOT-P8-A, C, D, E
- **size:** XL
- **brief:** "**HACKATHON DIFFERENTIATOR.** ERC-4337 validator that calls Automata DCAP on-chain (per `19_knowledge_base.md §4.4`). User op signature = `(quote, ecdsaSig)`. Validator: verify quote → check `mrTd` allowlist → recover signer → check `reportData == sha256(pubkey)`. Pin BOTH `mrSigner` AND `mrTd`. Target: ≤ 2M gas (acceptable for hackathon), stretch ≤ 1M via RIP-7212."
- **pre_read:** `19_knowledge_base.md §1.3, §4.1-4.4, §4.7`
- **post_demo:** `D-P8-03`, `D-P8-04`, `D-P8-05`

## SLOT-P8-C: On-chain audit anchor + policy registry
- **Stories:** E16-S7, E16-S8
- **Modules:** `/contracts/AuditAnchor.sol`, `/contracts/PolicyRegistry.sol`, `/crates/mandate-onchain/src/anchor.rs`
- **blocked_by:** SLOT-P3-A, P2-A
- **parallel_with:** SLOT-P8-A, B, D, E
- **size:** M
- **brief:** "Daily Merkle root → `AuditAnchor.sol` event. Cost target: <$0.01 on Base. Policy registry (compatible with Verax / EAS schema)."
- **pre_read:** `19_knowledge_base.md §4.10`
- **post_demo:** `D-P8-06`, `D-P8-07`

## SLOT-P8-D: Sponsor demo scripts + live attack demos
- **Stories:** E15-S5, E11-S8, E16-S9
- **Modules:** `/demo-scripts/sponsors/`, `/demo-scripts/red-team/`, `/demo-agents/research-agent/`, `/web-ui/src/routes/attestation-monitor/`
- **blocked_by:** SLOT-P8-A, B, C
- **parallel_with:** SLOT-P8-E (after others ready)
- **size:** L
- **brief:** "Per-sponsor demo scripts (Coinbase, Safe, AA, Verax/EAS, Automata). Real research-agent harness plus red-team scripts: prompt-injection deny, policy bypass, key exfil attempt, tampering detect, kill switch. Live attestation visualization in Web UI."
- **post_demo:** `D-P8-08..14`

## SLOT-P8-E: ENS + EAS + Verax integration
- **Stories:** E16-S11, E16-S12
- **Modules:** `/crates/mandate-core/src/identity/ens.rs`, `/crates/mandate-onchain/src/eas.rs`
- **blocked_by:** SLOT-P8-C
- **parallel_with:** SLOT-P8-A, B, C, D
- **size:** M
- **brief:** "Agent identity via ENS subname (Namestone or NameWrapper with burned fuses per `19_knowledge_base.md §4.9`). Verax / EAS attestation publishing."
- **pre_read:** `19_knowledge_base.md §4.9, §4.10`
- **post_demo:** `D-P9-09`, `D-P9-10` (cross-phase)

---

# Phase 9 — 4 paralelné sloty

## SLOT-P9-A: Marketplace pilot
- **Stories:** E16-S4
- **Modules:** `/examples/marketplace-buyer/`, `/examples/marketplace-seller/`
- **blocked_by:** SLOT-P4-B (live chain)
- **parallel_with:** SLOT-P9-B, C, D
- **size:** L
- **brief:** "Buyer agent calls seller agent's API. Seller emits x402 challenge. Buyer pays via vault. Reputation gates. End-to-end demo."
- **post_demo:** `D-P9-01`, `D-P9-02`

## SLOT-P9-B: ZK proof of policy eval (stretch)
- **Stories:** E16-S10
- **Modules:** `/zk-circuits/`, `/crates/mandate-zk/`
- **blocked_by:** SLOT-P2-A
- **parallel_with:** SLOT-P9-A, C, D
- **size:** XL
- **brief:** "RISC Zero or SP1 zkVM execution of policy eval. On-chain verifier accepts ZK proof without revealing full policy state. Use case: privacy-preserving policy evaluation."
- **pre_read:** `19_knowledge_base.md §1.3` (SP1 path on Automata)
- **post_demo:** `D-P9-03`

## SLOT-P9-C: Quality (fuzz, red team, coverage)
- **Stories:** E17-S1, E17-S2, E17-S3, E17-S4
- **Modules:** `/fuzz/`, `/tests/integration/`, `/docs/red-team/`
- **blocked_by:** SLOT-P8-D done
- **parallel_with:** SLOT-P9-A, B, D
- **size:** L
- **brief:** "cargo-fuzz targets for APRP, x402, policy compiler, decision token. 24h corpus run nightly. Integration test suite with full payment flow + error paths. Internal red team exercise per all 25 attacks in `04_threat_model.md`. Coverage > 80% on critical crates."
- **pre_read:** `04_threat_model.md`, `19_knowledge_base.md §5.9`
- **post_demo:** `D-P9-04`, `D-P9-05`, `D-P9-06`, `D-P9-07`

## SLOT-P9-D: Mobile PWA approval
- **Stories:** E11-S9
- **Modules:** `/mobile-pwa/`
- **blocked_by:** SLOT-P6-A, P6-B
- **parallel_with:** SLOT-P9-A, B, C
- **size:** M
- **brief:** "Installable PWA (manifest + service worker). Push notification reception. WebAuthn-based approval signing (mobile biometrics)."
- **post_demo:** `D-P9-08`

---

# Phase 10 — 4 paralelné sloty

## SLOT-P10-A: Appliance image
- **Stories:** E14-S4
- **Modules:** `/appliance-image/`
- **blocked_by:** SLOT-P7-D
- **parallel_with:** SLOT-P10-B, C, D
- **size:** XL
- **brief:** "Coreboot/Tianocore + minimal Linux + mandate preinstalled. Bootovateľný USB. TPM enabled, encrypted disk."

## SLOT-P10-B: External security audit
- **Stories:** E17-S5
- **Modules:** `/audit-reports/`
- **blocked_by:** SLOT-P9-C
- **parallel_with:** SLOT-P10-A, C, D
- **size:** XL
- **brief:** "Engagement: Trail of Bits / Zellic / Cure53. Report public. All critical/high findings remediated pre v1.0."

## SLOT-P10-C: Helm chart
- **Stories:** E14-S6
- **Modules:** `/helm/mandate/`
- **blocked_by:** SLOT-P7-D
- **parallel_with:** SLOT-P10-A, B, D
- **size:** M
- **brief:** "Helm chart deployment. PVC for state. HSM via DaemonSet (preconditions documented)."

## SLOT-P10-D: Business / docs / release
- **Stories:** E18-S1, E18-S2, E18-S3, E15-S6, E15-S7
- **Modules:** `/announcement/`, `/docs/editions.md`, `/business-plan/`, `/docs/runbooks/`, `/docs/api/`
- **blocked_by:** SLOT-P10-B
- **parallel_with:** SLOT-P10-A, B, C
- **size:** L
- **brief:** "Open source announcement (HN, Twitter, GitHub release v1.0.0). Editions matrix. Pricing model + LOI. Runbooks (install, recovery, key rotation, incident, audit export, upgrade). API reference docs (OpenAPI + mdbook)."

---

## §13 Integration / orchestrator slots

Cross-cutting changes (e.g. adding new error code, changing audit event format) **never** belong to a single slot. Use orchestrator-managed integration slots:

### INTEGRATION-CONTRACTS-CHANGE
- **Trigger:** Implementation needs new field in APRP, new error code, new event type, schema migration.
- **Process:** Update `17_interface_contracts.md` first; orchestrator approves; then change propagates to all affected slots in next iteration.

### INTEGRATION-PHASE-MERGE
- **Trigger:** End of phase. All in-phase slots have passed their demos.
- **Process:** Run `bash demo-scripts/run-phase.sh PX` end-to-end (not just per-slot). Fix any cross-slot integration bugs. Tag release.

### INTEGRATION-RED-TEAM
- **Trigger:** End of phases P3, P5, P8.
- **Process:** Spawn `Explore` subagent to audit attack vectors per `04_threat_model.md`. Result feeds back as new stories.

---

## §14 Spawning template (orchestrator)

When orchestrator spawns a slot agent, use this template:

```
Agent(
  description: "SLOT-PN-X: <slot-name>",
  subagent_type: "general-purpose",
  prompt: """
  You are implementing SLOT-PN-X of the mandate project.

  ## Pre-read (mandatory)
  - /Users/danielbabjak/Desktop/agent-vault-os/17_interface_contracts.md
  - /Users/danielbabjak/Desktop/agent-vault-os/26_end_to_end_implementation_spec.md
  - /Users/danielbabjak/Desktop/agent-vault-os/19_knowledge_base.md (sections X, Y)
  - /Users/danielbabjak/Desktop/agent-vault-os/<other phase-specific files>

  ## Stories to implement
  <list from 12_backlog.md>

  ## Allowed modules
  <list — do NOT touch other files>

  ## Acceptance gates
  Run: bash demo-scripts/run-single.sh D-PN-NN ... (must all pass)

  ## Constraints
  - Follow §10 forbidden patterns in 17_interface_contracts.md
  - Use only crates listed in 19_knowledge_base.md §5
  - Hand back a summary: what was implemented, demos passed, any deviations

  ## Failure handling
  If a demo fails, fix the implementation (NOT the demo). If you believe demo is wrong, escalate — do not modify demo scripts.
  """,
  run_in_background: true
)
```

---

## §15 Slot dependency graph (visual)

```
P0:    [01] [02] [03→01] [04→01] [05→01]
        │    │     │       │       │
P1:     ────┴─────┴───────┴───────┴────
        │                              │
        ├── A (APRP+SDK) ──┐           │
        ├── B (Gateway)    │           │
        ├── C (Signing)    │           │
        └── D (Mock x402) ─┘           │
                                       │
P2:     ├── A (Policy) ──── needs P1-A │
        ├── B (Budget)                 │
        └── C (x402 verifier)          │
                                       │
P3:     ├── A (Audit) ──── needs P0-04 │
        ├── B (Emergency) ─ needs P3-A │
        └── C (Approval CLI) needs P3-A│
                                       │
P4:     ├── A (Simulator)              │
        ├── B (Chain) ── needs P4-A    │
        └── C (Settlement)             │
                                       │
P5:     ├── A (PKCS#11)                │
        ├── B (TPM)                    │
        ├── C (Multisig)               │
        └── D (MCP)                    │
                                       │
P6:     ├── A (Web UI)                 │
        ├── B (Push)                   │
        ├── C (RBAC+webhook+digest)    │
        └── D (Docs)                   │
                                       │
P7:     ├── A (Self-attest)            │
        ├── B (TDX) ── needs P7-A      │
        ├── C (SEV) ── needs P7-A      │
        └── D (Packaging)              │
                                       │
P8:     ├── A (Safe module) ── P7-B/C  │ HACKATHON
        ├── B (4337 validator) P7-B/C  │ HERO
        ├── C (Anchor + registry)      │ PHASE
        ├── D (Demos+RT) ── P8-A,B,C   │
        └── E (ENS+EAS) ── P8-C        │
                                       │
P9:     ├── A (Marketplace)            │
        ├── B (ZK) — stretch           │
        ├── C (Quality)                │
        └── D (PWA)                    │
                                       │
P10:    ├── A (Appliance)              │
        ├── B (Audit)                  │
        ├── C (Helm)                   │
        └── D (Business+docs+release)  │
```

---

## §16 Estimated agent count per phase

(Assuming 1 agent per slot. Can compress if some agents handle multiple slots sequentially.)

| Phase | Slots | Min agents (parallel) |
|---|---|---|
| P0 | 5 | 5 (or 1 sequential) |
| P1 | 4 | 4 |
| P2 | 3 | 3 |
| P3 | 3 | 3 (B and C wait for A) |
| P4 | 3 | 3 (B waits for A) |
| P5 | 4 | 4 |
| P6 | 4 | 4 |
| P7 | 4 | 4 (B, C wait for A) |
| P8 | 5 | 5 (D waits for A,B,C) |
| P9 | 4 | 4 |
| P10 | 4 | 4 |

**Peak parallelism: 5 agents (P8).** Average ~4. Can scale up by splitting sub-tasks within slots.
