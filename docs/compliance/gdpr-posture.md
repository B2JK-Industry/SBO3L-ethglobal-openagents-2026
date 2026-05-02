# GDPR posture

> **Status:** 🟡 data-flow map + retention policy drafted; DPA template needed for B2B EU customers. **Not certified** (GDPR doesn't have certification — but Article 42 codes-of-conduct or BCRs would substitute, none in place).

## Scope

GDPR (EU 2016/679) applies to **personal data processing** in connection with offering services to people in the EU. SBO3L's controllership posture depends on where the deployment lives:

| Deployment | SBO3L role | Customer role | GDPR scope |
|---|---|---|---|
| SaaS (hypothetical post-hackathon) | Processor | Controller | Customer signs DPA (Art. 28) |
| Self-hosted by customer | _none_ | Controller | Customer is sole controller; SBO3L is software vendor (no GDPR role) |
| Hackathon demo (today) | _none — no production traffic_ | _none — no real users_ | N/A; capsule fixtures contain no personal data |

This document covers the SaaS-mode posture (the most regulated case).

## Personal data SBO3L processes

SBO3L's design **minimizes** personal data — the capsule schema is structurally focused on agent identity (ENS/Ed25519 pubkey), policy hash, and signed receipts. Personal data only enters at three points:

| Where | What | Lawful basis | Retention |
|---|---|---|---|
| Bearer token / JWT `sub` claim | Agent operator's customer-side user ID (opaque string) | Art. 6(1)(b) — contract performance | Lifetime of API access; deleted within 30 days of access revocation |
| Audit row `agent_id` | Agent's ENS name (e.g. `agent-001.customer.eth`) | Art. 6(1)(b) | Per customer retention policy (default 7 years per audit-log standard) |
| Sponsor adapter responses | Whatever the sponsor returns (typically tx hashes, execution IDs) | Art. 6(1)(f) — legitimate interest in audit trail | Same as audit row |

**SBO3L does NOT process:**
- ❌ Email addresses, names, postal addresses
- ❌ Cookies (the marketing site is static; no analytics; no tracking)
- ❌ Browser fingerprints
- ❌ IP addresses outside short-lived rate-limit buckets (deleted within 1h)
- ❌ Behavioral profiles
- ❌ Location data
- ❌ Special categories (Art. 9): health, biometric, political, religious, sexual

## Article 30 — records of processing activities (RoPA)

| Field | Value |
|---|---|
| Controller name | _(customer)_ |
| Processor name | SBO3L Inc. (post-hackathon entity) |
| Joint controllers | None |
| DPO contact | _none required at current scale (Art. 37 thresholds not met)_ |
| Processing purposes | (1) Authentication; (2) Policy enforcement; (3) Audit logging |
| Categories of data subjects | Agent operators (technical users) |
| Categories of personal data | Opaque user IDs (in JWT `sub`); ENS names (public-by-design) |
| Recipients | Customer-side audit consumers; ENS resolvers (public DNS-equivalent) |
| Cross-border transfers | None inside SBO3L; ENS resolution may resolve via globally-distributed RPC nodes |
| Retention period | Configurable; default 7 years post-last-activity per audit-log standard |
| Security measures | TLS in transit; bcrypt password storage; Ed25519 signing; chain-hashed audit log; multi-tenant isolation (V010); encryption at rest (V011 — gap, in flight) |

## Data subject rights (Articles 15–22)

| Right | SBO3L support | Notes |
|---|---|---|
| Right of access (Art. 15) | ✅ via `sbo3l audit query --agent-id <id>` | Returns full audit history for that agent |
| Right to rectification (Art. 16) | ⚠️ **structurally limited** — audit chain is append-only by design | Corrections appended as new audit rows referencing the prior; original NOT deleted (audit integrity) |
| Right to erasure (Art. 17) | ⚠️ **structurally limited** — see "Right to be forgotten" below | |
| Right to restriction (Art. 18) | ✅ — per-agent disable via PolicyReceipt revocation | |
| Right to portability (Art. 20) | ✅ — capsule export via `sbo3l passport export` | JSON format; trivially portable |
| Right to object (Art. 21) | ✅ via API access revocation | |
| Automated decision-making (Art. 22) | ✅ — every policy decision is human-readable + appealable | The PolicyReceipt itself is the explanation; deny-codes are categorized |

### Right to be forgotten (Art. 17) — special handling

The audit chain is **immutable by design** — that's the entire trust proposition. Naive deletion of an audit row breaks the `chain_hash_v2` linkage and invalidates every subsequent capsule.

Our approach (Art. 17(3)(b) — necessary for compliance with a legal obligation; Art. 17(3)(e) — establishment, exercise or defence of legal claims):
1. **Tombstone, not delete** — replace the personal-data field in the audit row with a deterministic placeholder (e.g. `<redacted-by-DSAR-2026-05-02>`); recompute `payload_hash` for that row only; insert a follow-up row that signs the redaction event.
2. **Preserve linkage** — the `chain_hash_v2` recomputes correctly because the redacted row's `payload_hash` becomes the new canonical hash for that row.
3. **Document the redaction** — the audit chain now contains both the redaction-claim and the proof-of-redaction; an external auditor can verify the redaction was authorized.

This satisfies "erasure" in a way that is **stronger** than typical SaaS (which deletes silently): the redaction itself is auditable.

**Limitation:** if a row's contents are themselves required for legal-obligation retention (Art. 17(3)(b)), redaction may be denied with a cited legal basis. SBO3L surfaces this as `dsar.denied_legal_obligation` in the response.

## Cross-border data transfers (Chapter V)

SBO3L is not a hosted SaaS today — there is no cross-border transfer at the SBO3L layer.

If/when SBO3L offers SaaS, the relevant transfer mechanisms would be:
- **EU↔US:** EU-US Data Privacy Framework (post-2023 ruling) for self-certified processors; or Standard Contractual Clauses 2021 module 2 (controller-to-processor).
- **EU↔third countries:** SCCs + a Transfer Impact Assessment (TIA) per Schrems II.

The customer is the controller and is responsible for their own TIA covering ENS resolver location (which may route through any global DNS node).

## Data Processing Agreement (DPA) — template

A DPA template will live at `docs/compliance/dpa-template.md` — **not yet drafted** (gap; estimate: 1 week with legal review).

The template will cover:
- Subject matter, duration, nature, purpose of processing
- Types of personal data + categories of data subjects
- Controller obligations
- Processor obligations (including subprocessor list)
- Audit rights
- Data subject rights cooperation
- Notification of data breach (≤ 72h to controller)
- Return / deletion of personal data at end of contract
- Subprocessor list (initial: AWS / GCP for SaaS deployment; `dalek-cryptography` and `secp256k1` for crypto primitives — these don't process personal data, but listing them is best practice)

## Breach notification (Art. 33–34)

| Trigger | SLA | Channel |
|---|---|---|
| Likely-breach detection | ≤ 24h to internal incident comms | Cascade-watch + Heidi alerts |
| Confirmed breach | ≤ 72h to controller (per Art. 33(2)) | Email + GitHub Security Advisory if customer is OSS |
| High-risk breach to data subjects | "without undue delay" (per Art. 34(1)) | Customer responsibility; SBO3L provides forensic support |

The audit chain itself is a **forensic asset** for breach analysis — an attacker that compromises a daemon cannot rewrite the audit chain without breaking the linkage hash, so the audit chain contains a tamper-evident record of what happened.

## DPO & supervisory authority

| Question | Answer |
|---|---|
| Is a DPO required? | No (Art. 37 thresholds not met at current scale) |
| Lead supervisory authority | TBD — will be the EU-member-state authority where SBO3L's main establishment is located post-incorporation |
| Public contact for data subjects | `privacy@sbo3l.dev` (provisioned at submission time) |

## Honest gaps

- 🔴 **No DPA template** — required before any B2B EU contract.
- 🔴 **No cookie banner / privacy policy on marketing site** — currently fine because there's no analytics; will need to add at SaaS launch if any cookies are introduced.
- 🟡 **Encryption at rest** — V011 work in flight.
- 🟡 **No formal subprocessor list** — easy to write, just hasn't been written.
- 🟡 **No Privacy Impact Assessment (PIA)** template — SaaS deployment will need one.

## See also

- [`README.md`](README.md) — top-level compliance posture.
- [`soc2-readiness.md`](soc2-readiness.md) — overlaps via CC6 (logical access) + C1 (confidentiality).
- [`audit-log-as-evidence.md`](audit-log-as-evidence.md) — Art. 30 RoPA support.
