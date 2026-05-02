# HIPAA gap analysis

> **Status:** 🟡 Gap analysis drafted; covered-entity scope mapped. **No BAA available.** Encryption-at-rest gap is blocking — V011 work needed before signing any HIPAA-scope contract.

## Scope question

HIPAA (Health Insurance Portability and Accountability Act, US) applies when PHI (Protected Health Information) is processed by a **Covered Entity** (CE) or **Business Associate** (BA).

| If the customer is | And SBO3L is used to | SBO3L role | HIPAA scope |
|---|---|---|---|
| A health plan / provider / clearinghouse (CE) | Process agent decisions about PHI | BA (requires BAA) | Yes |
| A non-CE B2B (e.g. marketing platform with no PHI) | Process non-PHI agent decisions | _none_ | No |
| Self-hosted internally | Process anything | _software vendor only_ | No (customer is sole CE/BA) |

This document covers the **BA-scope** posture: what an SBO3L-as-BA would need to satisfy.

## Per-rule analysis

### Privacy Rule (45 CFR 164.502–164.514)

| Requirement | Status | Notes |
|---|---|---|
| Use + disclosure limited to TPO + permitted purposes | ✅ | SBO3L processes only what the customer sends; no secondary use. Documented in the (forthcoming) BAA. |
| Minimum necessary standard | ✅ | Capsule schema is structurally minimal (no PHI-typical fields like name/DOB/SSN). |
| Marketing + sale prohibitions | ✅ | SBO3L does not market or sell customer data. Codified in BAA. |
| Notice of Privacy Practices (NPP) | _customer responsibility_ | CE provides NPP to data subjects; BA is not in NPP scope. |

### Security Rule (45 CFR 164.302–164.318) — administrative safeguards

| Requirement | Status | Evidence | Gap |
|---|---|---|---|
| §164.308(a)(1)(i) — Security Management Process | 🟡 | Threat model in `SECURITY_NOTES.md`; chaos suite | No formal annual risk analysis |
| §164.308(a)(1)(ii)(A) — Risk Analysis | 🔴 | — | Required; not done |
| §164.308(a)(1)(ii)(B) — Risk Management | 🟡 | F-1/F-2/F-3/F-5 controls + cascade-watch | No formal risk-management plan |
| §164.308(a)(1)(ii)(C) — Sanction Policy | 🔴 | — | N/A at 1-person scale |
| §164.308(a)(1)(ii)(D) — Information System Activity Review | ✅ | **The audit chain.** Every action logged + tamper-evident. |
| §164.308(a)(2) — Assigned Security Responsibility | 🟡 | Daniel = security officer (de facto) | No documented role assignment |
| §164.308(a)(3) — Workforce Security | 🔴 | — | N/A at 1-person scale |
| §164.308(a)(4) — Information Access Management | ✅ | Multi-tenant isolation (V010); JWT `sub` binding |
| §164.308(a)(5) — Security Awareness + Training | 🔴 | — | N/A at 1-person scale |
| §164.308(a)(6) — Security Incident Procedures | 🟡 | Chaos suite + Heidi escalation | No formal incident response plan |
| §164.308(a)(7) — Contingency Plan | 🟡 | Backup-restore drill (chaos-1); SQLite WAL | No formal DR plan |
| §164.308(a)(8) — Evaluation | 🔴 | — | Annual evaluation required |
| §164.308(b)(1) — BAA with subcontractors | 🔴 | — | Subprocessor BAAs needed if SaaS launches with subs |

### Security Rule — physical safeguards (§164.310)

These are **delegated to cloud DC** (Vercel + customer-side infra). SBO3L's role is to ensure cloud vendors are HIPAA-eligible:
- **Vercel:** offers HIPAA BAA on Enterprise plans — would need upgrade for production.
- **AWS / GCP:** both offer HIPAA-eligible services with BAA.
- **Customer self-host:** customer's responsibility.

| Requirement | Status |
|---|---|
| §164.310(a) — Facility Access Controls | _delegated to cloud DC_ |
| §164.310(b) — Workstation Use | _customer-side scope_ |
| §164.310(c) — Workstation Security | _customer-side scope_ |
| §164.310(d) — Device + Media Controls | _customer-side scope_ |

### Security Rule — technical safeguards (§164.312)

| Requirement | Status | Evidence | Gap |
|---|---|---|---|
| §164.312(a)(1) — Access Control | ✅ | F-1 default-deny + JWT `sub` binding |
| §164.312(a)(2)(i) — Unique User Identification | ✅ | Each agent has Ed25519 pubkey + ENS name |
| §164.312(a)(2)(ii) — Emergency Access Procedure | 🟡 | Admin-bypass on branch protection (logged) | No formal break-glass procedure |
| §164.312(a)(2)(iii) — Automatic Logoff | 🟡 | JWT TTL configurable | No default-aggressive timeout |
| §164.312(a)(2)(iv) — Encryption + Decryption | 🟡 | TLS in transit | **Encryption at rest GAP — V011** |
| §164.312(b) — Audit Controls | ✅ | **The hash-chained audit log is, structurally, an exemplary §164.312(b) control.** See `audit-log-as-evidence.md`. |
| §164.312(c)(1) — Integrity | ✅ | Capsule + audit chain hash linkage |
| §164.312(c)(2) — Mechanism to Authenticate ePHI | ✅ | Ed25519 signing of every audit row |
| §164.312(d) — Person or Entity Authentication | ✅ | F-1 bearer + JWT |
| §164.312(e)(1) — Transmission Security | ✅ | TLS only |
| §164.312(e)(2)(i) — Integrity Controls | ✅ | TLS + chain-hashed audit log |
| §164.312(e)(2)(ii) — Encryption | ✅ | TLS (+ encryption at rest gap noted above) |

### Breach Notification Rule (§164.400–164.414)

| Requirement | SLA | SBO3L posture |
|---|---|---|
| Discovery of breach | ≤ 60 days to CE for unsecured PHI | Cascade-watch detection + 60-day formal SLA in BAA |
| ≥ 500-individual breach | ≤ 60 days to HHS Secretary | CE's responsibility; BA assists |
| Media notification | If ≥ 500 in a state | CE's responsibility |

## The encryption-at-rest gap

This is the **single largest gap** for HIPAA scope. Current state:

- ✅ TLS in transit (covered)
- ✅ Application-layer signing (covered)
- 🔴 SQLite database files on disk are **not encrypted at rest**

V011 closes this:
- Plan: SQLCipher integration (drop-in SQLite replacement with AES-256 page encryption)
- Estimate: 2 weeks engineering + customer-side key management UX
- Workaround for non-V011-bridge customers: deploy SBO3L on filesystem-level-encrypted storage (LUKS / dm-crypt / EBS encryption with customer-managed KMS keys). This satisfies the HIPAA requirement without code changes but offloads complexity to the customer.

## BAA template

A BAA template is **not yet drafted**. Required clauses (per §164.504(e)(2)):

- Permitted + required uses + disclosures
- Use of PHI for management + administration of BA
- Reporting obligations (breach notification SLA)
- BA's safeguards
- Subcontractor BAAs
- Customer audit rights
- Termination provisions
- Return or destruction of PHI at termination

Estimate: 2 weeks draft + 1 week legal review.

## Risk acceptance for non-V011 deployments

A customer that **knowingly accepts the encryption-at-rest gap** (e.g. self-host on filesystem-encrypted storage) can run SBO3L in HIPAA-scope today, but must:
1. Sign a BAA with explicit risk-acceptance clause for app-layer encryption-at-rest.
2. Document filesystem-encryption configuration.
3. Manage the disk-encryption keys via their own KMS.
4. Accept that an SBO3L-side restoration drill (chaos-1) re-creates the SQLite file in the encrypted filesystem context.

This is a defensible posture for a customer with strong cloud encryption capabilities (AWS EBS + KMS, GCP Persistent Disk + CMEK).

## Honest summary

| Question | Answer |
|---|---|
| Can a customer use SBO3L for HIPAA-scope workloads today? | Only with risk acceptance + filesystem-encrypted storage workaround. |
| What's the timeline to "yes, sign a BAA"? | ~3 months: V011 (2 weeks) + BAA draft (3 weeks) + risk analysis (1 week) + auditor review (4 weeks). |
| What's the structural advantage SBO3L brings? | The audit chain is exemplary §164.312(b). The signed PolicyReceipt is exemplary §164.312(c)(2). The capsule format is exemplary §164.312(e)(2)(ii). |

## See also

- [`soc2-readiness.md`](soc2-readiness.md) — overlapping CC6 / CC7 controls.
- [`gdpr-posture.md`](gdpr-posture.md) — overlapping data-subject rights handling (audit-row tombstoning approach is the same).
- [`audit-log-as-evidence.md`](audit-log-as-evidence.md) — §164.312(b) walkthrough.
