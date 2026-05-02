# Compliance posture — SBO3L

> **Phase 3.7 — Compliance.** This directory captures SBO3L's posture against the four compliance frameworks most commonly required of AI/agent infrastructure used in regulated industries: SOC 2 Type 1, GDPR, HIPAA, and PCI-DSS. Each document is **honest disclosure**, not a marketing pitch — gaps are called out explicitly with timeline-to-close estimates.

> **Maturity at hackathon time (2026-05-02):** SBO3L is hackathon-scope demo software. **None** of the four frameworks below are formally certified. The documents in this directory describe **readiness posture** — which controls already exist in code, which are gaps that would need to close before a real audit, and what the work-to-close looks like. A production deployment would commission a SOC 2 Type 1 audit (Drata / Vanta scan) before going live with regulated workloads.

## Why compliance work fits SBO3L's thesis

SBO3L's core proposition is **a cryptographically-verifiable trust boundary** — every agent action produces a tamper-evident audit row before any side effect. That architecture is **structurally aligned** with most compliance requirements around audit logging, segregation of duties, and immutable change records.

Concretely, the same hash-chained Ed25519-signed audit log that verifies the `/proof` page also satisfies:

- **SOC 2 CC7.2** — system monitoring + audit logging.
- **GDPR Art. 30** — records of processing activities.
- **HIPAA §164.312(b)** — audit controls.
- **PCI-DSS 10.x** — track and monitor all access to network resources.

Capsules + `chain_hash_v2` linkage are the **artifact** that satisfies "tamper-evident" in all four standards, with the verifier `sbo3l verify-audit --strict-hash` providing the runtime check.

## Documents in this directory

| Doc | Framework | Status | Audit-readiness |
|---|---|---|---|
| [`soc2-readiness.md`](soc2-readiness.md) | SOC 2 Type 1 | 🟡 readiness map drafted; no audit | ~6–9 months to Type 1 |
| [`gdpr-posture.md`](gdpr-posture.md) | GDPR | 🟡 data-flow map + retention policy | DPA template needed for B2B EU customers |
| [`hipaa-gap-analysis.md`](hipaa-gap-analysis.md) | HIPAA | 🟡 gap analysis; covered-entity scope mapped | BAA needed; encryption-at-rest gap (V010 work) |
| [`pci-dss-scope.md`](pci-dss-scope.md) | PCI-DSS | 🟢 SBO3L is **out-of-scope by design** | Customer's PCI scope unaffected if they segregate cardholder data outside SBO3L |
| [`shared-controls.md`](shared-controls.md) | (cross-cutting) | 🟢 reusable control inventory | Controls satisfying multiple frameworks |
| [`audit-log-as-evidence.md`](audit-log-as-evidence.md) | (cross-cutting) | 🟢 mapping SBO3L audit chain to standards' control IDs | Reference doc for auditor walkthroughs |

## Honest gaps

The single biggest gap across all four frameworks is **encryption at rest**. Today, SBO3L stores the audit log in SQLite without disk encryption. V010 (multi-tenant isolation, #208) is a prerequisite for the encryption-at-rest work since the migration touches the same tables. Estimate: 2 weeks engineering + 2 weeks auditor review post-Phase 4.

The second-biggest gap is **personnel controls** (SOC 2 CC1.4, GDPR Art. 32(4)). SBO3L is a 1-person hackathon project today; SOC 2 Type 1 needs documented hiring, onboarding, training, and termination procedures, which are not relevant at this scale. Closing this gap is a hiring + ops question, not an engineering one.

## Customer-facing FAQ

> **Q: Is SBO3L SOC 2 certified?**
> A: Not yet. We have a readiness map (`soc2-readiness.md`) showing which controls already exist. Formal Type 1 audit is on the roadmap for the post-hackathon phase if customer demand materializes.

> **Q: Can I use SBO3L with PHI / cardholder data / EU PII?**
> A: Today, only with a documented risk acceptance. The framework-specific docs in this directory tell you exactly what gaps you'd be accepting. We are happy to walk through any of them with your compliance team.

> **Q: Does SBO3L's audit chain satisfy [auditor's question]?**
> A: Probably yes — see `audit-log-as-evidence.md` for the standards mapping. If your auditor needs something specific, open a GitHub Discussion and we'll write it up.

## See also

- [`SECURITY.md`](../../SECURITY.md) — top-level security policy + bug bounty.
- [`SECURITY_NOTES.md`](../../SECURITY_NOTES.md) — internal deployment hardening notes.
- [`docs/security/`](../security/) — disclosure policy details, PGP key, out-of-scope.
