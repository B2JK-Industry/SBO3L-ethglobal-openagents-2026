# Compliance audit scan readiness (R14 P5)

> **Status:** procurement playbook + tool-by-tool readiness map. **Actual scans are Daniel-side** — Drata / Vanta / Scout-Suite all require account creation + cloud credentials that an automated agent can't provision.

## Scope of this doc

R14 P5 asked for "Drata or Vanta scan; Scout-Suite for AWS/GCP audit; GDPR scan; HIPAA gap report; final docs/compliance/scan-results-2026-05-XX.md."

The honest position: **none of these scans can run today** because:
- SBO3L has no production cloud deployment yet (no AWS / GCP / Azure account with KMS provisioned).
- Drata / Vanta require business email + customer onboarding (~1 week ramp).
- Scout-Suite requires AWS IAM credentials with `ReadOnly` policy attached.

What ships **today** in this PR:
1. **Per-tool readiness map** — what each scanner would produce if run today, what gaps it would flag, what it would cost.
2. **Procurement playbook** — for each tool: free trial path, business-email requirement, expected onboarding time, cost.
3. **Self-scan checklist** — what Daniel can do *without* commercial tooling to surface most of what an audit would find.

## Tool-by-tool readiness

### Drata

> Continuous-compliance platform. Automates SOC 2 / HIPAA / ISO 27001 evidence collection.

| Field | Value |
|---|---|
| Free trial | 14 days (sales-gated) |
| Business email required | ✅ |
| Setup time | ~1 week (integrations: AWS, GCP, GitHub, Slack, identity provider) |
| Estimated cost | $7K-$12K/year (Series-A pricing tier) |
| Output format | Web dashboard + audit-ready evidence package |

**Predicted findings (based on R13 P7 readiness map):**
- 🔴 Personnel controls (CC1.x) — N/A at 1-person scale; would advise hiring procedure docs before scaling.
- 🔴 Encryption at rest — V011 work needed.
- 🟡 SBOM (Software Bill of Materials) — would advise `cargo cyclonedx` + `syft`.
- 🟡 MFA enforcement — would advise enforcing on GitHub org + cloud.
- ✅ Audit logging — Drata would map our chain to CC7.2 automatically.
- ✅ Change management — branch protection + CI gate satisfies CC8.

### Vanta

> Direct competitor to Drata. Same scope (SOC 2 / HIPAA / ISO 27001). Marginally better for GDPR.

| Field | Value |
|---|---|
| Free trial | 14 days |
| Business email required | ✅ |
| Setup time | ~1 week |
| Estimated cost | $8K-$14K/year |
| Output format | Web dashboard + customer-facing trust report |

**Predicted findings:** identical to Drata above. Vanta's differentiator is the **public-facing trust report** (SBO3L would link from `sbo3l.dev/security`); Drata's differentiator is depth of technical evidence collection.

### Scout-Suite

> Multi-cloud security audit (AWS, GCP, Azure). Open-source from NCC Group. Free.

| Field | Value |
|---|---|
| Free | ✅ open-source |
| Cloud credentials required | ✅ (`ReadOnly` IAM role / GCP `roles/viewer`) |
| Setup time | 30 min once cloud account exists |
| Cost | $0 + ~$5/month for the IAM scan role |
| Output format | Static HTML report |

**Predicted findings (when SBO3L production cloud exists):**
- Network: no VPC flow logs (default); should enable.
- IAM: bastion-host access patterns; depends on production deploy.
- Storage: S3 / GCS bucket policies; will check for public-readable keys.
- Encryption: KMS key rotation policy; should be ≤ 1 year.
- Logging: CloudTrail / Cloud Audit Logs scope; should be `All` not just `Read`.

**Today's status:** N/A — no production cloud account.

### GDPR scan tools (alternatives)

| Tool | Free | What it produces |
|---|---|---|
| **Termly** | freemium | Cookie banner + privacy policy generator |
| **Iubenda** | freemium | DPA template + cookie management |
| **OneTrust** | $5K+/year | Full GDPR program management |

**SBO3L's GDPR posture today** (per `gdpr-posture.md`):
- ✅ Data minimization — capsule schema excludes PII by design.
- ✅ Article 30 RoPA — drafted, awaits SaaS onboarding to populate.
- 🔴 DPA template — gap; ~1 week with legal.
- 🟡 Privacy policy on marketing site — currently fine (no analytics → no cookies → no banner needed); will need at SaaS launch.

### HIPAA gap report tools

| Tool | Free | What it produces |
|---|---|---|
| **HIPAA One** | $300/scan | Self-assessment scan + remediation plan |
| **Compliancy Group** | $5K+/year | Annual gap analysis + workplace-of-the-year program |
| **HHS OCR Risk Assessment Tool** | Free (HHS-provided) | Lite gap report; not auditor-grade |

**SBO3L's HIPAA gap today** (per `hipaa-gap-analysis.md`):
- 🔴 Encryption at rest (V011 work)
- 🔴 BAA template — ~3 weeks draft + legal
- 🔴 Personnel safeguards — N/A at 1-person scale
- ✅ §164.312(b) audit controls — exemplary
- ✅ §164.312(c)(1) integrity — chain hash linkage

## Self-scan checklist (no commercial tools)

Daniel can run all of these today **without** Drata/Vanta/Scout-Suite to surface most of what a real audit would find:

### Repository hygiene

- [x] Dependabot enabled (covers dep CVEs)
- [ ] `cargo audit` in CI — TODO
- [ ] `npm audit` in CI on integration packages — TODO
- [ ] `cargo cyclonedx` SBOM generation in CI — TODO
- [x] Branch protection on `main` (admin bypass logged via cascade-watch)
- [x] Signed commits encouraged (commit-coauthored-by trailer model)
- [ ] MFA enforcement on GitHub org — Daniel-side (GitHub org settings)

### Secret scanning

- [x] GitHub secret scanning auto-enabled (default for public repos)
- [ ] `trufflehog` or `gitleaks` in CI — TODO
- [ ] Pre-commit hook to scan staged secrets — TODO
- [x] No secrets in `git log` (verified via `git-secrets --scan-history` in earlier round)
- [x] PyPI 2FA backup codes stored locally (per `pypi_2fa_recovery_codes.md`)

### Code-level security

- [x] cargo-fuzz harnesses (5 targets — `fuzz/`)
- [x] proptest invariants (`crates/sbo3l-core/tests/proptest_invariants.rs`)
- [x] cargo-mutants weekly (`mutation-testing.yml`)
- [x] chaos engineering suite (5 scenarios)
- [x] Default-deny auth (F-1)
- [x] Idempotent state machine (F-3)
- [x] Hash-chained audit log (CC7.2 / §164.312(b) exemplary)

### Infrastructure (when cloud exists)

- [ ] Encryption at rest enabled — V011 gap
- [ ] KMS key rotation ≤ 1 year — pending production deploy
- [ ] VPC flow logs / equivalent — pending production deploy
- [ ] CloudTrail / GCP Audit Logs scope = All — pending production deploy

### Compliance docs

- [x] `SECURITY.md` (R13 P6)
- [x] `docs/security/out-of-scope.md`
- [x] `docs/compliance/` 7 docs (R13 P7)
- [ ] DPA template (GDPR gap) — TODO
- [ ] BAA template (HIPAA gap) — TODO
- [ ] Privacy policy + cookie banner (when SaaS launches with analytics) — TODO

## Procurement playbook — recommended order

If Daniel wants to graduate from "ready" to "audited":

1. **Week 1 — preparation** (free):
   - Run the self-scan checklist above end-to-end.
   - Enable `cargo audit` + `npm audit` + `cargo cyclonedx` in CI.
   - Generate SBOM for current release (v1.2.0).

2. **Week 2-3 — engage** (sales calls):
   - Drata vs Vanta: pick based on cost + GDPR coverage need.
   - Sign up for free trial (14 days).
   - Set up integrations: AWS/GCP, GitHub, Slack, identity provider.

3. **Week 4-12 — evidence collection** (Drata/Vanta automated):
   - Most CC2-CC9 controls auto-collect from integrations.
   - CC1 (personnel) requires manual upload of HR docs.
   - C1 (confidentiality) maps to data classification — document at `docs/compliance/data-classification.md`.

4. **Week 12+ — audit** (auditor-led):
   - Pick Type 1 (point-in-time) for first audit; faster + cheaper than Type 2.
   - Auditor reviews evidence package + walks through controls.
   - 4-6 weeks to attestation.

**Total cost** (year 1): ~$10-15K platform + ~$15-25K auditor = $25-40K. Realistic for a Series A startup; not for a hackathon project.

## Final R14 status

🟡 **Procurement playbook + readiness map ship today.** Actual scans + audit attestations are post-hackathon roadmap — they require commercial sign-up + production cloud + auditor engagement that can't happen in the submission window.

The **honest claim** to judges: SBO3L has the **artifact** (audit chain) that satisfies the most-scrutinized control across all four major frameworks (SOC 2 CC7.2, GDPR Art. 30, HIPAA §164.312(b), PCI-DSS Req 10). The other controls are mapped + gaps documented; closing them is engineering + procurement work that follows naturally from the existing posture.

## See also

- [`README.md`](README.md) — top-level compliance posture.
- [`soc2-readiness.md`](soc2-readiness.md) — TSC walkthrough (CC1-CC9 + A1 + C1).
- [`gdpr-posture.md`](gdpr-posture.md) — Article 30 RoPA + data subject rights.
- [`hipaa-gap-analysis.md`](hipaa-gap-analysis.md) — Privacy + Security + Breach Notification rules.
- [`pci-dss-scope.md`](pci-dss-scope.md) — out-of-scope-by-design.
- [`audit-log-as-evidence.md`](audit-log-as-evidence.md) — auditor walkthrough script.
- [`shared-controls.md`](shared-controls.md) — cross-framework control inventory.
