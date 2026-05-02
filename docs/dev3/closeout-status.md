# Dev 3 — closeout status report

Rounds 13 + 14 (final 2 rounds of the hackathon). Earlier rounds covered in
[`docs/dev3/TRIAGE.md`](./TRIAGE.md) (round-11 audit) and the per-round
TRIAGE deltas. This report consolidates everything Dev 3 shipped after the
SBO3L rebrand.

## Round 13 — design + first-pass production code

| PR | Title | Brief mapping | Status |
|---|---|---|---|
| #280 | i18n Latin batch (DE/FR/IT/ES/PT-BR/PL/CS/HU) | R13 P4a | merged |
| #284 | i18n RTL/CJK (AR/HE/ZH-CN/ZH-TW/RU/UK/TR/HI/TH) + RTL helper | R13 P4b | merged |
| #287 | SOC 2 / GDPR / HIPAA / PCI-DSS posture docs | R13 P7 | merged |
| #288 | recharts decision viz on /admin/audit | R13 P3 | merged |
| #290 | Monaco policy editor at /t/[slug]/admin/policy/edit | R13 P6 | merged |
| #291 | OG image + Twitter card + DocSearch runbook | R13 P7 | merged |
| #294 | Production design docs (Postgres+RLS, Stripe, Mobile) | R13 P78 | merged |
| #296 | i18n Japanese (21st locale) | R13 P4c | merged |
| #297 | Per-tenant billing UI at /t/[slug]/admin/billing | R13 P9 | merged |

**Round 13 totals:** 9 PRs merged. 21 locales total (EN+SK+KO+JA + Latin8 +
RTL/CJK9). Recharts visualization, Monaco editor, OG/Twitter cards, three
production design docs, billing UI scaffold, compliance posture matrix.

## Round 14 — design docs → real code

| PR | Title | Brief mapping | Status |
|---|---|---|---|
| #309 | Mobile Expo skeleton — apps/mobile/ (tabs + push + scanner + biometric) | R14 P3 | merged |
| #311 | Stripe wired in test mode (lib + 3 routes + UpgradeButton + tests) | R14 P2 | merged |
| #315 | Postgres backend behind --features postgres + V020 schema + RLS | R14 P1 | merged |
| #317 | Operator console — date filters + CSV/JSONL export + sbo3l-client | R14 P4 | merged |
| #318 | Marketing polish — Cmd+K + animated hero + screenshot CI triggered | R14 P5 | merged |

**Round 14 totals:** 5 PRs merged. Every R14 brief P-bucket addressed at
least partially. Honest scope cuts documented in
[`docs/dev3/scope-cut-report.md`](./scope-cut-report.md).

## Round-13 + Round-14 grand total

**14 PRs merged** across the two rounds. Net additions:

- 1 brand-new Astro/RN app (apps/mobile)
- 5 new admin pages (audit / users / keys / flags / policy editor / billing)
- 17 new locale dictionaries (10 Latin/Cyrillic/Devanagari/Thai/Turkish/Japanese
  + 9 RTL/CJK)
- 4 production design docs + 1 deploy howto
- Stripe Checkout + webhook + Customer Portal API surface
- Postgres schema + RLS policies + sqlx-postgres connection helper
- recharts decision viz + CSV/JSONL export with filter dimensions
- Mobile companion app skeleton (8 routes, push registration, biometric
  approval gate)

## What's deliberately NOT in this report

Earlier-round work (R1–R12) including:
- Marketing site bring-up, /demo flow, /proof WASM verifier
- Trust DNS visualization
- Marketplace UI
- Multi-tenant scaffold (#270)
- Versioned docs + VersionSelector
- Hosted-app Vercel deploy + GitHub Actions CI
- /submission/<bounty> per-partner pages
- Korean (KO) i18n + EN/SK baseline
- /demo step-3 playground (tamper + visualize + samples)
- ArchDiagram v2 + OpenAPI Redoc + LocaleSwitcher

Those PRs are catalogued in the per-round TRIAGE deltas. Anyone reviewing
the submission should treat this report as "what closed out the rebrand
era" rather than "everything Dev 3 ever shipped."
