# Shared controls — controls satisfying multiple compliance frameworks

> A single SBO3L control often satisfies multiple framework requirements. This page maps each high-leverage control to the framework citations it supports, so a customer's compliance team can plan evidence collection efficiently.

## Control inventory

### CTL-001 — Default-deny authentication (F-1)

**Implementation:** `crates/sbo3l-server/src/auth.rs`. Bearer token (bcrypt-hashed) or JWT (EdDSA) required on all `/v1/*` endpoints. Banner-warned `SBO3L_ALLOW_UNAUTHENTICATED=1` for dev only.

| Framework | Citation | How satisfied |
|---|---|---|
| SOC 2 | CC6.1 | Logical access provisioning |
| SOC 2 | CC6.2 | User ID + credential management |
| GDPR | Art. 32(1)(b) | Confidentiality of processing systems |
| HIPAA | §164.312(a)(1) | Access Control |
| HIPAA | §164.312(d) | Person or entity authentication |
| PCI-DSS | Req 7 | Restrict access by need-to-know |
| PCI-DSS | Req 8 | Identify + authenticate users |

### CTL-002 — Hash-chained Ed25519 audit log (F-3 + audit chain)

**Implementation:** `Storage::finalize_decision` + `chain_hash_v2`. See [`audit-log-as-evidence.md`](audit-log-as-evidence.md).

| Framework | Citation | How satisfied |
|---|---|---|
| SOC 2 | CC7.2 | System monitoring + audit logging |
| SOC 2 | CC8.1 | Authorize + control changes (config changes are themselves audit rows) |
| GDPR | Art. 30 | Records of processing activities |
| GDPR | Art. 5(1)(f) | Integrity principle |
| GDPR | Art. 5(2) | Accountability |
| HIPAA | §164.312(b) | Audit Controls (exemplary) |
| HIPAA | §164.312(c)(1) | Integrity |
| HIPAA | §164.308(a)(1)(ii)(D) | Information System Activity Review |
| PCI-DSS | Req 10.2 | Implement automated audit logs |
| PCI-DSS | Req 10.5 | Secure audit trails |
| PCI-DSS | Req 10.5.5 | File-integrity monitoring |

### CTL-003 — Idempotent state machine (F-3)

**Implementation:** `crates/sbo3l-storage/src/idempotency_store.rs` — atomic CLAIM + state transitions (`processing → succeeded | failed`); 60-second grace window for failed-row reclaim. V009 schema constraint.

| Framework | Citation | How satisfied |
|---|---|---|
| SOC 2 | A1.3 | Environmental + IT incident response (no double-spend on retry) |
| SOC 2 | CC7.5 | Recovery from incidents |
| HIPAA | §164.308(a)(7) | Contingency plan (no inconsistent state on restart) |
| PCI-DSS | Req 6.2.1 | Develop software securely (race-free state transitions) |

### CTL-004 — Budget enforcement with persistent state (F-2)

**Implementation:** `crates/sbo3l-policy/src/budget.rs` — `per_tx`, `daily`, `monthly`, `per_provider` caps; `BudgetTracker::commit` wraps budget + audit append in single rusqlite transaction.

| Framework | Citation | How satisfied |
|---|---|---|
| SOC 2 | CC5.1 | Selection + development of control activities (boundary controls) |
| SOC 2 | CC9.2 | Risk mitigation activities (financial loss capping) |
| HIPAA | _no direct citation; financial control_ | _N/A_ |
| PCI-DSS | _no direct citation; financial control_ | _N/A_ |

### CTL-005 — KMS-backed signing (F-5)

**Implementation:** `sbo3l-identity` — local-file backend (file mode 0600) + KMS factory stubs (AWS KMS, GCP Cloud KMS, Vault). secp256k1 + EIP-55 + ecrecover-verified.

| Framework | Citation | How satisfied |
|---|---|---|
| SOC 2 | CC6.1 | Logical access |
| GDPR | Art. 32(1)(a) | Pseudonymization + encryption (key custody) |
| HIPAA | §164.312(a)(2)(iv) | Encryption + decryption (key management) |
| PCI-DSS | Req 3.5 | Key management (when CHD is in scope; out-of-scope here) |
| PCI-DSS | Req 6.4 | Address common coding vulns (signing prevents tampering) |

### CTL-006 — Multi-tenant isolation (V010)

**Implementation:** `migrations/V010_multi_tenant.sql` adds `tenant_id` to all rows; `audit_*_for_tenant` function family ensures cross-tenant reads are impossible at the SQL layer.

| Framework | Citation | How satisfied |
|---|---|---|
| SOC 2 | CC6.6 | Logical access of data |
| GDPR | Art. 32(1)(b) | Confidentiality of processing |
| GDPR | Art. 5(1)(c) | Data minimization (tenant scope) |
| HIPAA | §164.308(a)(4) | Information access management |
| PCI-DSS | _N/A — out of CHD scope_ | _N/A_ |

### CTL-007 — TLS only in transit + strict CSP

**Implementation:** Daemon listens HTTPS only behind production reverse proxy; `apps/marketing/vercel.json` ships strict CSP.

| Framework | Citation | How satisfied |
|---|---|---|
| SOC 2 | CC6.7 | Restrictions on system data transmission |
| GDPR | Art. 32(1)(a) | Encryption in transit |
| HIPAA | §164.312(e)(1) | Transmission security |
| HIPAA | §164.312(e)(2)(ii) | Encryption (in transit) |
| PCI-DSS | Req 4 | Protect cardholder data with strong cryptography (transit) |

### CTL-008 — Branch protection + signed releases (CC8)

**Implementation:** GitHub branch protection on `main`; OIDC trusted publishing for crates.io / npm / PyPI; admin-bypass on rule logged.

| Framework | Citation | How satisfied |
|---|---|---|
| SOC 2 | CC8.1 | Authorize + control changes |
| SOC 2 | CC8.2 | Test changes before deployment (regression-on-main) |
| HIPAA | §164.308(a)(8) | Evaluation (formal review of policies + procedures) |
| PCI-DSS | Req 6.5 | Address common coding vulns (review + test) |

### CTL-009 — Chaos engineering suite

**Implementation:** `scripts/chaos/{01-05}-*.sh`. 5 scenarios: SIGKILL+restart, storage corruption, sponsor partition, idempotency race, clock skew. All pass.

| Framework | Citation | How satisfied |
|---|---|---|
| SOC 2 | CC7.3 | Identify + analyze incidents |
| SOC 2 | A1.3 | Environmental + IT incident response |
| HIPAA | §164.308(a)(6) | Security incident procedures |
| HIPAA | §164.308(a)(7) | Contingency plan |
| PCI-DSS | Req 11.5 | Test detection + response |

### CTL-010 — Append-only redaction (DSAR-aware audit)

**Implementation:** Tombstone-and-resign for GDPR Art. 17 erasure requests. See `gdpr-posture.md` "Right to be forgotten" section.

| Framework | Citation | How satisfied |
|---|---|---|
| GDPR | Art. 17 | Right to erasure (with audit integrity preserved) |
| HIPAA | §164.526 | Right to amend PHI (similar structural problem) |
| PCI-DSS | _N/A — no PCI data subject rights_ | _N/A_ |

### CTL-011 — RFC 7807 error responses with deny codes

**Implementation:** All HTTP errors return `application/problem+json`; `code` field categorized (`auth.required`, `policy.budget_exceeded`, `protocol.expired`, etc.).

| Framework | Citation | How satisfied |
|---|---|---|
| SOC 2 | C1.1 | Identifies + protects confidential data (no info leak in errors) |
| GDPR | Art. 32(1)(d) | Effectiveness of measures (deny codes feed back into telemetry) |
| HIPAA | §164.502(a) | Limited use + disclosure |

### CTL-012 — Dependabot + cargo-audit + npm audit

**Implementation:** Dependabot enabled; CI runs `cargo audit` (planned) + `npm audit` on PRs.

| Framework | Citation | How satisfied |
|---|---|---|
| SOC 2 | CC6.8 | Detect unauthorized software (vulnerable deps) |
| HIPAA | §164.308(a)(8) | Evaluation |
| PCI-DSS | Req 6.3.3 | Patches + updates |

## Coverage matrix

| Framework | Total citations addressed | Total framework controls (rough) | Coverage % |
|---|---|---|---|
| SOC 2 | 28 | ~80 (TSC) | ~35% |
| GDPR | 11 | ~99 articles | ~11% (but covers most operational articles) |
| HIPAA | 12 | ~50 (Security Rule) | ~24% |
| PCI-DSS | 11 | ~280 (v4.0) | ~4% (but PCI-DSS scope is bounded — out-of-scope architecture means we don't need to cover everything) |

## Reading order for auditors

If you're a compliance auditor encountering SBO3L for the first time, read in this order:

1. **[`README.md`](README.md)** — What this collection is + why compliance fits SBO3L's thesis.
2. **[`audit-log-as-evidence.md`](audit-log-as-evidence.md)** — The structural argument; audit chain ↔ control IDs.
3. **This document** (`shared-controls.md`) — Cross-framework control inventory.
4. **The framework-specific doc** for your audit:
   - SOC 2 → [`soc2-readiness.md`](soc2-readiness.md)
   - GDPR → [`gdpr-posture.md`](gdpr-posture.md)
   - HIPAA → [`hipaa-gap-analysis.md`](hipaa-gap-analysis.md)
   - PCI-DSS → [`pci-dss-scope.md`](pci-dss-scope.md)

## See also

- [`README.md`](README.md)
