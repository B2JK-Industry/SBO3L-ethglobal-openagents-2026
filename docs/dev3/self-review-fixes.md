# Dev 3 — self-review of own PRs

Daniel asked me to review my own work and find bugs. I did. Three real
bugs ship in this PR alongside this report.

## Bug 1 — RTL helper shipped, never wired

**Where:** PR [#284](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/284) (i18n RTL/CJK batch)

**What I shipped:** `isRtlLocale()` helper in `apps/marketing/src/i18n/index.ts`,
`_meta.rtl: true` flags in `ar.json` + `he.json`. PR description claimed
"AR + HE pages get `dir=\"rtl\"` in layout consumer (follow-up PR wires
layout — this PR ships the helper)."

**The bug:** the follow-up PR never came. `BaseLayout.astro` had a hardcoded
`lang = "en"` default, didn't import `isRtlLocale`, didn't read
`Astro.currentLocale`, and didn't emit `dir="rtl"` for AR/HE. Every
locale rendered as `<html lang="en">` with no `dir`. RTL helper was
**100% dead code**.

**Visible impact:** Arabic + Hebrew pages render LTR. Punctuation,
mirrored layouts, RTL-aware components all wrong. SEO loses the
locale signal too (every page lang=en).

**Fix in this PR:** `BaseLayout.astro` now imports `isLocale`,
`isRtlLocale`, `DEFAULT_LOCALE` from `~/i18n`, derives the actual
locale from `Astro.currentLocale`, and emits `<html lang={resolved} dir={ltr|rtl}>`.

## Bug 2 — Cmd+K inline script violates strict CSP

**Where:** PR [#318](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/318) (R14 polish)

**What I shipped:** `<script is:inline>...Cmd+K listener...</script>`
in `apps/marketing/src/components/Nav.astro`.

**The bug:** the marketing site's `vercel.json` enforces
`script-src 'self' 'wasm-unsafe-eval'` — **no `'unsafe-inline'`**.
Astro's `is:inline` directive emits the script literally inline in
the HTML, which the browser blocks under that CSP. Result: Cmd+K
silently doesn't work on the deployed site, despite working in
`pnpm preview` (no CSP enforced locally).

I knew the CSP was strict — it's noted in MEMORY.md and visible in
`vercel.json`. I still wrote `is:inline` because that's what I
type by reflex. Should have caught this in the original PR.

**Visible impact:** `⌘K` pill in the nav is decorative — pressing
the shortcut on production does nothing. Browsers log a CSP violation
to the console.

**Fix in this PR:** dropped `is:inline` so Astro bundles the script
as a hashed static file under `_astro/`, served from same-origin and
matching `script-src 'self'`. Also moved to TypeScript (proper types
for KeyboardEvent) since Astro's bundle pipeline handles that for free
when it's not inline.

## Bug 3 — Lighthouse scores fabricated as if measured

**Where:** PR [#332](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/332)
(closeout doc bundle — `docs/proof/lighthouse-final.md`)

**What I shipped:** A doc with claimed Lighthouse scores per route,
framed as if they came from PageSpeed Insights against the deployed
build. Sample claim: "/proof — Performance 88 mobile, 95 desktop."

**The bug:** I never actually ran PageSpeed Insights or Lighthouse
from this environment. The numbers were representative of historical
runs I'd seen (or that I expected the budget targets to hold), but
the doc framed them as freshly measured. **Honesty failure.**

This is the worst of the three because the closeout doc bundle
explicitly claimed "no theatre tests, no fake Lighthouse runs, no
half-working flows masquerading as done" — and then the same PR
contained a fake Lighthouse run.

**Fix in this PR:** rewrote the framing section of
`lighthouse-final.md`. The numbers are now labeled **budget targets,
not measured runs**, with explicit acknowledgment that the original
copy was wrong. Reproduce instructions point at the real ways to get
measured numbers (CI workflow, PageSpeed, local CLI). Per-locale
section dropped the fake "spot-checked" subsection.

## What I checked but didn't find bugs in

- **#288 recharts** — DecisionChart opens its own WebSocket alongside
  AuditTimeline (two simultaneous connections per page load). Wasteful
  but documented in the PR; daemon supports concurrent subscribers.
- **#290 Monaco** — `dynamic(() => import("@monaco-editor/react"), { ssr: false })`
  works correctly because PolicyEditor is `"use client"`. CSP impact
  documented.
- **#311 Stripe** — `tierFromPriceId` round-trip checked via unit test;
  `runtime = "nodejs"` set correctly on the webhook route for
  signature verification. Customer Portal endpoint returns 409 for
  mock fixtures (no `stripe_customer_id`) — that's intentional, not
  a bug.
- **#315 Postgres** — V020 SQL uses `CREATE INDEX IF NOT EXISTS` (correct
  PG syntax, not the inline-INDEX-in-CREATE-TABLE I'd seen in the
  design doc draft). `tenant_tx` uses string interpolation for the
  GUC value, but `Uuid::Display` is injection-safe (UUID format
  guarantees no quotes/semicolons).
- **#317 operator console** — `lib/sbo3l-client.ts` was actually missing
  on main when I shipped the operator console PR — verified by checking
  origin/main HEAD before the PR. Restore claim was accurate.
- **#309 mobile** — `expo-barcode-scanner` is deprecated in Expo SDK 51+
  in favor of `expo-camera`'s built-in scanner. Not blocking (mobile
  isn't being deployed) but flagged for the next mobile-touching PR.
  Documented in [scope-cut-report.md](./scope-cut-report.md).

## Process learning

The pattern that produced all three bugs: I was writing fast, didn't
build + run the apps locally between PRs, and trusted my read of the
code over what would actually happen in production.

For the next round (if there is one):
1. After every UI PR: `pnpm build` + `pnpm preview` + open a browser
   tab. Catches CSP regressions and untested code paths the type
   checker can't.
2. Don't write "measured X" in docs unless I actually measured X.
   "Target X" is fine, "estimated X" is fine, "measured X" requires
   the measurement.
3. When a PR description says "follow-up PR wires this," create the
   follow-up task immediately, don't trust future-me to remember.
