# SOC 2 Type 1 readiness map

> **Status:** 🟡 readiness map drafted. **Not audited.** This doc walks through the SOC 2 Trust Services Criteria (TSC) and maps each criterion to either an existing SBO3L artifact (✅), a partial control (🟡), or an open gap (🔴). A formal Type 1 audit (single-point-in-time control existence) requires ~6–9 months of evidence collection + an auditor-led scan (Drata / Vanta / Secureframe).

> **Scope:** SOC 2 has 5 TSC categories. SBO3L's core scope is **Security** (mandatory) + **Availability** + **Confidentiality**. Processing Integrity and Privacy are framework-relevant for some customer use cases — covered in `gdpr-posture.md` (Privacy) and `audit-log-as-evidence.md` (Processing Integrity).

## Readiness summary

| Category | Status | Controls in place | Gaps |
|---|---|---|---|
| CC1 — Control Environment | 🔴 1/5 | Code of conduct (implied via OSS license + CONTRIBUTING.md) | Personnel policies (hiring, training, performance review) — N/A at 1-person scale |
| CC2 — Communication | 🟡 3/5 | Public security policy (`SECURITY.md`); GitHub Discussions; release notes | Internal incident comms playbook, customer breach notification SLA |
| CC3 — Risk Assessment | 🟡 2/4 | Threat model implied in `SECURITY_NOTES.md`; chaos engineering (5 scenarios) | Formal annual risk assessment; vendor risk management |
| CC4 — Monitoring | ✅ 4/5 | regression-on-main.yml; cascade-watch; GitHub Security Alerts; uptime probes | SIEM aggregation across surfaces |
| CC5 — Control Activities | ✅ 5/6 | Code review (PR + codex); branch protection; signed commits (optional); separation of duties (4+1 agent model); F-1 default-deny auth | MFA enforcement on all systems |
| CC6 — Logical & Physical Access | 🟡 6/9 | F-1 bearer + JWT auth; audit log of every decision; KMS-backed signing (sbo3l-identity); env-var secrets; ENS-anchored agent identity | Encryption at rest (gap; pending V011); HSM-backed signing (KMS factory shipped, HSM not); physical security (cloud DC delegated) |
| CC7 — System Operations | ✅ 7/8 | Audit chain (CC7.2); chaos suite; idempotency state machine; budget enforcement; load test harness | Capacity planning baseline (gap; load-test in flight) |
| CC8 — Change Management | ✅ 6/6 | Branch protection; PR review; CI gate (regression-on-main); release-note CHANGELOG; signed releases (workflow-issued OIDC tokens for crates/PyPI/npm) | — |
| CC9 — Risk Mitigation | 🟡 3/5 | Disaster-recovery via SQLite + git; backup-restore validated by chaos-1; multi-region read replicas (planned) | DR drill cadence; incident postmortem template |
| **A1 — Availability** | 🟡 4/6 | Idempotency replay; backup persistence (SQLite WAL); uptime probes; chaos suite | SLO formalization; runbook completeness; error-budget tracking |
| **C1 — Confidentiality** | 🟡 3/5 | Capsule contains no PHI/PCI by design; ENS records public-by-design; redacted error messages (RFC 7807) | Encryption at rest (V011 gap); data classification table |

## Per-criterion detail

### CC1 — Control Environment

| ID | Description | Status | Evidence | Gap |
|---|---|---|---|---|
| CC1.1 | Demonstrates commitment to integrity & ethical values | 🟡 | OSS Apache-2.0 license; `CODE_OF_CONDUCT.md`; public release notes | No formal ethics committee |
| CC1.2 | Board independence + oversight | 🔴 | — | No board (1-person project at hackathon scale) |
| CC1.3 | Management organizational structure | 🔴 | Implicit (Daniel = sole lead) | Org chart needed at Series A+ scale |
| CC1.4 | Demonstrates commitment to attract, develop, retain competent individuals | 🔴 | — | Hiring/onboarding/training/termination procedures — not applicable at 1-person scale |
| CC1.5 | Holds individuals accountable for SOC 2 responsibilities | 🔴 | — | Performance review process — N/A at scale |

**Verdict:** CC1 is the largest gap and is fundamentally a hiring/ops question. Not engineering work to close.

### CC2 — Communication & Information

| ID | Description | Status | Evidence |
|---|---|---|---|
| CC2.1 | Internal communication of objectives | 🟡 | `docs/win-backlog/` Phase 1/2/3 ACs + ticket structure |
| CC2.2 | Internal communication of system changes | ✅ | `CHANGELOG.md`; release notes; cascade-watch events |
| CC2.3 | External communication (customers, vendors) | ✅ | `SECURITY.md`; GitHub release notes; FEEDBACK to KeeperHub |

### CC4 — Monitoring Activities

| ID | Description | Status | Evidence |
|---|---|---|---|
| CC4.1 | Periodic + ongoing monitoring | ✅ | `.github/workflows/regression-on-main.yml`, `.github/workflows/ccip-gateway-uptime.yml`, `cascade-watch` Monitor |
| CC4.2 | Internal control deficiencies evaluated + communicated | ✅ | codex automated PR review with P1/P2/P3 severity; GitHub Security Advisories |

### CC5 — Control Activities

| ID | Description | Status | Evidence |
|---|---|---|---|
| CC5.1 | Selection + development of control activities | ✅ | F-1 (auth), F-2 (budget persistence), F-3 (idempotency state machine), F-5 (signing), V001-V010 schema migrations |
| CC5.2 | Selection + development of general controls over technology | ✅ | Branch protection (admin-bypass off for production); signed releases (OIDC); reproducible builds |
| CC5.3 | Deploys policies + procedures | ✅ | `SECURITY.md`, `docs/release/`, `docs/runbook-rehearsal.md` |

### CC6 — Logical & Physical Access Controls

| ID | Description | Status | Evidence | Gap |
|---|---|---|---|---|
| CC6.1 | Logical access provisioning + restrictions | ✅ | F-1 default-deny auth; JWT `sub` claim binding; KMS-backed signing | — |
| CC6.2 | Manage user IDs + creds | 🟡 | Bearer token hashed (bcrypt); JWT pubkey-based | No password rotation policy |
| CC6.3 | Restrictions on privileged access | ✅ | RBAC via signed PolicyReceipts; per-call gating in Smart Wallet abstraction | — |
| CC6.4 | Physical access controls | 🟡 | Cloud DC (Vercel + customer-side) — delegated | No on-prem deploy yet |
| CC6.5 | Termination of access | 🔴 | — | Process N/A at 1-person scale |
| CC6.6 | Logical access of data | ✅ | Multi-tenant isolation V010 (#208); audit row records every read |
| CC6.7 | Restrictions on system data transmission | ✅ | TLS only; CSP on marketing; OIDC trusted publishing | — |
| CC6.8 | Detect + prevent unauthorized software | 🟡 | Dependabot; cargo-audit (planned); npm audit | No SBOM (gap) |

### CC7 — System Operations

| ID | Description | Status | Evidence |
|---|---|---|---|
| CC7.1 | System monitoring | ✅ | regression-on-main; uptime probes; observability dashboard (#252) |
| CC7.2 | Audit logs | ✅ | **SBO3L's defining feature.** Hash-chained Ed25519-signed audit log. See `audit-log-as-evidence.md`. |
| CC7.3 | Identify + analyze incidents | 🟡 | Chaos suite (5 scenarios); GH Issues for incident tracking | No incident postmortem template |
| CC7.4 | Respond to incidents | 🟡 | Cascade-watch alerts + Heidi automation | Customer-facing breach notification SLA |
| CC7.5 | Recovery from incidents | ✅ | Chaos-1 (DB persistence across SIGKILL); backup-restore drill |

### CC8 — Change Management

| ID | Description | Status | Evidence |
|---|---|---|---|
| CC8.1 | Authorize + control changes | ✅ | Branch protection; PR review (≥1 approval on main; admin-bypass tracked) |
| CC8.2 | Test changes before deployment | ✅ | regression-on-main blocks merges on CI red |
| CC8.3 | Document changes | ✅ | `CHANGELOG.md`; release notes; commit messages with co-author trailers |

### CC9 — Risk Mitigation

| ID | Description | Status | Evidence | Gap |
|---|---|---|---|---|
| CC9.1 | Identifies + evaluates risks | 🟡 | `SECURITY_NOTES.md` threat model; chaos suite | Annual formal risk review |
| CC9.2 | Mitigation activities | ✅ | F-1/F-2/F-3/F-5 controls in place | — |

### A1 — Availability

| ID | Description | Status | Evidence | Gap |
|---|---|---|---|---|
| A1.1 | System availability monitoring | ✅ | uptime probes; cascade-watch | — |
| A1.2 | Capacity management | 🟡 | Phase 3.4 load test harness (#261) | Capacity baseline TBD |
| A1.3 | Environmental + IT incident response | ✅ | Chaos suite | — |

### C1 — Confidentiality

| ID | Description | Status | Evidence | Gap |
|---|---|---|---|---|
| C1.1 | Identifies + protects confidential data | 🟡 | Capsule schema excludes PHI/PCI by design | No data classification table for customer PII |
| C1.2 | Disposal of confidential data | 🟡 | Audit retention policy (planned) | No formal data-purge SOP |

## Audit readiness checklist

To go from current state to Type 1 audit-ready:

- [ ] Engage auditor (Drata / Vanta / Secureframe) — 2-week setup
- [ ] Close encryption-at-rest gap (V011) — 2 weeks engineering
- [ ] Close MFA gap on all repo + cloud accounts — 1 week
- [ ] Close SBOM gap (cargo-cyclonedx, syft) — 1 week
- [ ] Document incident response runbook with named roles — 1 week
- [ ] Document data classification table — 1 week
- [ ] Run formal annual risk assessment — 1 week
- [ ] Auditor evidence collection — 6-8 weeks

**Total estimated time:** ~3 months from "go" decision to Type 1 attestation.

## See also

- [`gdpr-posture.md`](gdpr-posture.md)
- [`hipaa-gap-analysis.md`](hipaa-gap-analysis.md)
- [`pci-dss-scope.md`](pci-dss-scope.md)
- [`shared-controls.md`](shared-controls.md)
- [`audit-log-as-evidence.md`](audit-log-as-evidence.md)
