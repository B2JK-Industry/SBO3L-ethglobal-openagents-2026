# Phase 2 Frontend — Design Doc (pre-Phase-2 direction-setting)

> **Audience:** Daniel (review + approve), Dev 3 (Eve 🎨 + Frank 📚, future-self picking up the tickets), Heidi (test-plan implications).
> **Outcome:** alignment on tech stack, sitemap, design tokens, security posture, and per-ticket scaffolding before Phase 2 unlocks. Reduces wasted starts when CTI-3-1 lands and Dev 3's queue opens.
> **Status:** DRAFT — not a ticket, not normative. Pre-merge review changes welcome.

This doc covers the four Phase 2 frontend tickets owned by Dev 3:

| Ticket | Surface | Effort | Stack proposal | Section |
|---|---|---|---|---|
| [CTI-3-2](../win-backlog/06-phase-2.md#cti-3-2) | `sbo3l.dev` (marketing) | 12h | Astro 5 (static) | [§3](#3-cti-3-2--marketing-site) |
| [CTI-3-3](../win-backlog/06-phase-2.md#cti-3-3) | `docs.sbo3l.dev` (docs) | 16h | Astro 5 + Starlight | [§4](#4-cti-3-3--docs-site) |
| [CTI-3-4](../win-backlog/06-phase-2.md#cti-3-4) | `app.sbo3l.dev` (hosted) | 24h | Next.js 15 (App Router) | [§5](#5-cti-3-4--hosted-preview) |
| [T-3-5](../win-backlog/06-phase-2.md#t-3-5) | `app.sbo3l.dev/trust-dns` | 12h | Vite + D3 (embedded) | [§6](#6-t-3-5--trust-dns-visualization) |

It also previews two Phase-3 amplifier tickets that build on the same scaffolding so we don't accidentally architect them out of reach:

| Ticket | Surface | Section |
|---|---|---|
| [T-3-6](../win-backlog/06-phase-2.md#t-3-6) | `docs.sbo3l.dev/trust-dns` (1500-word essay) | [§7](#7-t-3-6--trust-dns-essay) |
| [ENS-MC-A1](../win-backlog/10-first-tier-amplifiers.md#ens-mc-a1) | `sbo3l.dev/trust-dns-story` (zine) | [§8](#8-ens-mc-a1--trust-dns-visual-zine) |

Out of scope (separate doc when those tickets unlock): T-3-7 submission narrative, ENS-MC-A2 manifesto, ENSIP draft pages, ERC-8004 mainnet UI surfaces.

---

## 1. Constraints inherited from `apps/marketing/` starter

The current starter (merged in PR #86, commit `8f30359`) sets several constraints we **must** preserve when we replace it:

1. **CSP-restricted, no external CDNs.** `vercel.json` ships `Content-Security-Policy: default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'`. That means:
   - No Google Fonts, no Adobe Fonts, no Typekit. Fonts are system stacks (`ui-sans-serif, -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif`) or self-hosted woff2 files emitted by the build.
   - No Tailwind CDN, no Bootstrap CDN, no jsDelivr. Tailwind (if used) compiles into a self-hosted CSS file at build time.
   - No analytics scripts (Google Analytics, Plausible-hosted, PostHog). If we want telemetry on `sbo3l.dev` we self-host or skip.
   - No external image hosts (no Imgur, no Unsplash hotlinks). Images live under `apps/marketing/public/` and ship `data:` URIs only when small enough to be inlined.
   - No third-party iframes (no Twitter embeds, no YouTube embeds on the marketing site itself; demo videos link out).
2. **Privacy-first.** Eve's standing rule 5 in `03-agents.md`: no third-party analytics. We honour it across all three sites.
3. **Lighthouse > 90.** Eve's rule 1, also CTI-3-2/CTI-3-3/T-3-5 acceptance criteria. Implies: small JS bundles, minimal blocking CSS, lazy images, no font CLS.
4. **WCAG AA.** Eve's rule 2. Implies contrast ratios audited against the brand palette below; semantic HTML; keyboard nav on every interactive element; aria labels on the architecture SVG (the starter already does this — keep that pattern).
5. **Mobile-first responsive.** No fixed widths; CSS grid + flexbox.
6. **JS budget < 200 KB gzipped per page.** Eve's rule 4. This is the single hardest constraint; see the per-stack budget breakdowns below.
7. **Dark theme by default.** Starter is dark (`#0a0a0f`/`#e6e6ec`/`#4ad6a7`). We keep dark default and add an opt-in light theme via `prefers-color-scheme` + manual toggle on docs site (Frank's audience reads at all hours).
8. **Tagline preserved verbatim.** *Don't give your agent a wallet. Give it a mandate.* Lowercase "mandate". Identity locked (`01-identity.md`).
9. **No emoji in marketing copy.** `01-identity.md` voice section: emoji allowed in marketing site / demo video / per-agent persona docs. Use sparingly, never in section headers, never in CTAs.

If a Phase 2 PR violates any of the above, it should be rejected before review (Heidi gate).

---

## 2. Brand surface — design tokens (canonical for all four sites)

These tokens come from `apps/marketing/style.css` (the existing starter). They are the canonical values; CTI-3-2/3/4 + T-3-5 import them, do not redefine them.

```css
:root {
  /* colour */
  --bg:     #0a0a0f;   /* near-black, slightly cool */
  --fg:     #e6e6ec;   /* near-white, slightly cool */
  --muted:  #9999a8;   /* secondary text, captions */
  --accent: #4ad6a7;   /* signature mint-green; brand CTA + numbers */
  --code-bg: #14141c;
  --border:  #2a2a3a;

  /* layout */
  --max:     920px;    /* content width on marketing + docs prose */
  --max-app: 1280px;   /* wider on app dashboard */

  /* type */
  --font-sans: ui-sans-serif, -apple-system, BlinkMacSystemFont, "Segoe UI",
               system-ui, sans-serif;
  --font-mono: ui-monospace, "SF Mono", Menlo, Consolas, monospace;
  --line-height-prose: 1.55;
  --line-height-code:  1.4;

  /* type scale (1.25 modular) */
  --fs-0: 0.85rem;     /* small / captions */
  --fs-1: 1rem;        /* body */
  --fs-2: 1.15em;      /* lede */
  --fs-3: 1.7em;       /* h2 */
  --fs-4: clamp(1.8em, 4vw, 2.6em);  /* hero h1 */

  /* radii + spacing follow 4-px grid */
  --r-sm: 4px;  --r-md: 8px;  --r-lg: 12px;
}
```

**Light-theme variant** (docs site only, opt-in via `data-theme="light"` attribute set by JS):

```css
[data-theme="light"] {
  --bg:     #ffffff;
  --fg:     #1a1a25;
  --muted:  #5a5a6a;
  --accent: #1a8b6c;   /* darker mint to clear AA on white */
  --code-bg: #f5f5fa;
  --border:  #e0e0e8;
}
```

All four sites import these via a shared package `@sbo3l/design-tokens` (see [§9](#9-shared-package--sbo3ldesign-tokens)).

**Contrast spot-check (WCAG AA — 4.5:1 minimum for normal text):**
- `--fg #e6e6ec` on `--bg #0a0a0f` → ~17.4:1 ✓ AAA
- `--muted #9999a8` on `--bg #0a0a0f` → ~7.4:1 ✓ AAA
- `--accent #4ad6a7` on `--bg #0a0a0f` → ~10.6:1 ✓ AAA
- `--accent #4ad6a7` text on `--fg #e6e6ec` background (used only in btn.primary inverted state) → ~1.6:1 ✗ — keep accent as background-only when foreground is light
- light `--accent #1a8b6c` on `#ffffff` → ~4.6:1 ✓ AA (tight; bumping to `#168065` would clear AAA — open question for Daniel)

---

## 3. CTI-3-2 — Marketing site

### 3.1 Stack: Astro 5 (static-only)

Astro because:

- **Zero JS by default.** Pages render static HTML; islands of interactivity opt-in. Aligns with the < 200 KB JS budget — most marketing pages can ship 0 KB JS.
- **Content collections** (Astro 5) give us typed `src/content/blog/*.md` and `src/content/case-studies/*.md` with schema validation. Frank can author markdown without touching the framework.
- **No client-side framework forced.** We can drop in a Solid/Preact island for interactive pricing comparisons or tabbed code blocks if needed; we don't pay for one we don't use.
- **Self-hosted everything.** Astro's build emits all assets locally; CSP `default-src 'self'` works out of the box.

Rejected alternatives:
- **Next.js** for marketing — too much framework for what is essentially 5-7 static pages. App Router's RSC/server-action model is overkill, ships ~80 KB framework JS even for static pages.
- **SvelteKit** — viable, smaller bundle, but Astro's content collections + native MDX win for marketing+docs unification.
- **Hand-roll HTML (the current starter)** — fine for one page. CTI-3-2 expands to 5+ pages with shared layout, blog, case studies. Hand-rolling that means re-implementing Astro poorly.

### 3.2 Sitemap

```
sbo3l.dev/
├── /                          (index — hero, numbers, what-we-do, evidence, CTA)
├── /features                  (architecture diagram, per-pillar deep-dive cards)
├── /pricing                   (free / team / enterprise placeholder; "coming soon")
├── /proof                     (capsule download + verifier instructions; replaces github.io redirect)
├── /trust-dns-story           (Astro page; lazy loads zine — ENS-MC-A1)
├── /blog/                     (content collection; T-3-6 + ENS-MC-A2 land here as MD)
│   ├── /trust-dns-essay       (T-3-6 mirror at docs)
│   └── /trust-dns-manifesto   (ENS-MC-A2)
└── /case-studies/             (placeholder; Phase 3 fills if pilots land)
```

### 3.3 Page-by-page wireframes (ASCII)

#### `/` (index) — extends the starter, reorganises for narrative arc

```
┌──────────────────────────────────────────────────────────────┐
│ NAV  [SBO3L]    Features  Docs  Proof  Blog  GitHub          │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│   HERO                                                       │
│   ────                                                       │
│   Don't give your agent a wallet.                            │
│   Give it a mandate.                                         │
│                                                              │
│   [lede paragraph — current starter copy verbatim]           │
│                                                              │
│   [ Start in 5 minutes ]   [ View source ]                   │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│   NUMBERS STRIP (live values, not hardcoded — see §3.5)      │
│   ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐            │
│   │ 377/377 │ │  13/13  │ │   8/8   │ │ 3 live  │            │
│   │  tests  │ │  gates  │ │ adverse │ │  paths  │            │
│   └─────────┘ └─────────┘ └─────────┘ └─────────┘            │
├──────────────────────────────────────────────────────────────┤
│   WHAT SBO3L IS  (text + 7-step pipeline list — from starter)│
├──────────────────────────────────────────────────────────────┤
│   ARCHITECTURE  (the SVG sequence diagram — keep it)         │
├──────────────────────────────────────────────────────────────┤
│   WHAT SBO3L BLOCKS  (11-item adversarial list — from starter)│
├──────────────────────────────────────────────────────────────┤
│   LIVE EVIDENCE  (ENS / Uniswap / KH outputs — from starter) │
├──────────────────────────────────────────────────────────────┤
│   REPRODUCE YOURSELF  (curl + cargo example — from starter)  │
├──────────────────────────────────────────────────────────────┤
│   "TRUST DNS" TEASER  (NEW — links to /trust-dns-story)      │
│   one paragraph + thumbnail of zine page 1                   │
├──────────────────────────────────────────────────────────────┤
│   RESOURCES  (link list — from starter, expanded)            │
├──────────────────────────────────────────────────────────────┤
│ FOOTER — minimal: GitHub link, ETHGlobal mention, license    │
└──────────────────────────────────────────────────────────────┘
```

Net add vs starter: numbers become live (build-time injection), Trust DNS teaser, separate /features /pricing /blog routes pull off detail panels.

#### `/features`

```
HERO STRIP: "Six pillars. One signed envelope."

GRID (2x3 on desktop, 1-column on mobile):
  [APRP wire format     ]  [Hash-chained audit ]  [Self-contained capsule]
  [Sponsor adapter trait]  [ENS as trust DNS   ]  [No-key agent boundary  ]

each card:
  - icon (custom SVG, no icon font)
  - 1-line claim
  - 3-bullet "what this means in code" with file:line references
  - "Read the spec →" link to docs.sbo3l.dev/concepts/<pillar>

ARCHITECTURE DEEP-DIVE (full-width SVG, larger than home page version)
  - clickable nodes navigate to the relevant docs page
```

#### `/pricing`

Placeholder per CTI-3-2 spec; `01-identity.md` doesn't define pricing. Three-card layout:

```
[ FREE             ]   [ TEAM            ]   [ ENTERPRISE      ]
  $0 / month            (coming soon)          (talk to us)
  Self-host            Hosted                  On-prem + KMS
  Single user          Up to 25 agents         Unlimited
  Community support    Email support           SLA + audit help
  [Get started]        [Join waitlist]         [Contact]
```

Honest framing — no fake quote forms. "Get started" is GitHub. "Join waitlist" is a `mailto:` (no third-party form service, again CSP).

#### `/proof`

Replaces the current `/proof` → github.io redirect with a real page. Embeds:

- One-paragraph explainer of what a Passport capsule is + how to verify.
- Download buttons for a sample capsule (`sbo3l-passport-sample.json`) and the verifier binary instructions.
- Live "verify in browser" widget (interactive — only JS island on the marketing site): paste a capsule, see structural + strict-hash verification result. Deferred decision: the verifier today is Rust, not WASM. If WASM compile is non-trivial, this page links out to the CLI verifier and the github.io page rather than embedding a live verifier. **Open question for Daniel** ([§10 Q1](#10-open-questions-for-daniel)).

### 3.4 Layout architecture (Astro)

```
apps/marketing/
├── astro.config.mjs           (output: 'static', no SSR)
├── src/
│   ├── content/               (typed collections)
│   │   ├── config.ts          (zod schemas for blog + case-studies)
│   │   ├── blog/
│   │   │   ├── trust-dns-manifesto.md     (T-3-6)
│   │   │   └── trust-dns-manifesto.md (ENS-MC-A2)
│   │   └── case-studies/
│   ├── layouts/
│   │   ├── BaseLayout.astro   (head, nav, footer; ALL pages extend this)
│   │   └── BlogPost.astro     (article wrapper)
│   ├── components/
│   │   ├── Nav.astro
│   │   ├── Footer.astro
│   │   ├── NumberStrip.astro
│   │   ├── ArchDiagram.astro  (the SVG; reusable)
│   │   ├── EvidenceBlock.astro
│   │   ├── CodeBlock.astro    (syntax-highlight via Shiki, build-time, 0 KB JS)
│   │   └── ProofVerifier.tsx  (only interactive island; deferred — see §10 Q1)
│   ├── pages/
│   │   ├── index.astro
│   │   ├── features.astro
│   │   ├── pricing.astro
│   │   ├── proof.astro
│   │   ├── trust-dns-story.astro      (ENS-MC-A1 — interactive zine)
│   │   ├── blog/[...slug].astro
│   │   └── case-studies/[...slug].astro
│   └── styles/
│       ├── global.css         (imports @sbo3l/design-tokens)
│       └── prose.css          (typography for blog posts)
├── public/
│   ├── og-image.png           (1200×630 social card)
│   ├── favicon.svg
│   ├── sample-capsule.json    (downloadable on /proof)
│   └── trust-dns-zine/        (ENS-MC-A1 artist deliverables)
├── package.json
└── README.md                  (replaces current starter README)
```

### 3.5 Live numbers strip — build-time injection

Today: hardcoded `377/377`, `13/13`. Stale within a week.

Proposed: `apps/marketing/src/data/stats.json` regenerated by a CI step before build. Astro reads it at build time → strip updates with every deploy.

```ts
// apps/marketing/src/data/build-stats.ts
import stats from './stats.json';
export const liveStats = {
  testCount:        stats.test_count,        // wc -l of cargo test --workspace output, parsed
  demoGates:        stats.demo_gates,        // 13/13 — counted from demo-scripts/run-openagents-final.sh exit codes
  adversarialGates: stats.adversarial_gates, // 8/8 — counted from adversarial test list
  livePathCount:    stats.live_path_count,   // 3 (KH + ENS + Uniswap) — hardcoded for now, becomes 4 with 0G
};
```

CI: `.github/workflows/marketing-stats.yml` runs nightly + on main push, executes `scripts/collect-marketing-stats.sh`, commits `stats.json` if changed. If we don't want a write-back commit, the script runs at deploy time (`vercel-build` script in `package.json`). Recommend deploy-time variant — simpler, no git churn. **Open question Q2 below.**

### 3.6 JS budget projection

| Page | Astro framework | Islands | Total JS gzipped |
|---|---|---|---|
| `/` | 0 KB (static) | 0 | ~0 KB |
| `/features` | 0 KB | 0 | ~0 KB |
| `/pricing` | 0 KB | 0 | ~0 KB |
| `/proof` | ~3 KB hydration | ProofVerifier (Preact, ~12 KB) | ~15 KB |
| `/blog/*` | 0 KB | 0 | ~0 KB |
| `/trust-dns-story` | ~3 KB | Zine controller (~8 KB) | ~11 KB |

Comfortably under 200 KB / page. The Preact island on `/proof` only hydrates on user interaction (`client:visible`).

### 3.7 Acceptance-criteria mapping

| AC from CTI-3-2 | How this design satisfies it |
|---|---|
| `sbo3l.dev` resolves | Vercel project, `sbo3l.dev` apex + `www.` redirect (Dev 4 / Daniel for DNS) |
| Lighthouse perf > 90 | Static Astro, no external CDNs, system fonts, < 15 KB JS per page |
| WCAG AA compliant | Tokens contrast-checked above; semantic HTML; aria labels on SVG; keyboard nav |
| Mobile responsive | CSS grid + flexbox, mobile-first breakpoints (`@media (max-width: 640px)` already in starter) |
| No external CDN deps | CSP `default-src 'self'` enforced via `vercel.json`; Astro build is self-contained |

---

## 4. CTI-3-3 — Docs site

### 4.1 Stack: Astro Starlight

Frank's standing rule 1 (`03-agents.md` line 250) names Astro Starlight explicitly. No competing recommendation needed.

Why Starlight over Docusaurus:
- **Same Astro toolchain as marketing** → shared `@sbo3l/design-tokens`, shared CodeBlock component, no duplicate build infra.
- **Built-in search (Pagefind)** is local + offline + zero-JS-on-load — meets CSP + privacy.
- **MDX-first** → Frank ships markdown, he doesn't fight a framework.
- **i18n-ready** — Phase 3 if we want JP/KR docs for sponsor outreach.

### 4.2 Sitemap

```
docs.sbo3l.dev/
├── /                          (welcome — audience picker: dev / compliance / judge)
├── /quickstart                (5-min tutorial — port of QUICKSTART.md)
├── /concepts/                 (conceptual guides)
│   ├── /aprp                  (APRP wire format)
│   ├── /audit-log             (hash-chained audit)
│   ├── /capsule               (Passport capsule v2)
│   ├── /policy                (deterministic policy decision)
│   ├── /budget                (multi-scope budget)
│   ├── /sponsor-adapters      (GuardedExecutor trait)
│   └── /trust-dns             (T-3-6 — Trust DNS essay, 1500 words)
├── /sdks/
│   ├── /typescript            (auto-generated from JSDoc + handwritten guide)
│   └── /python                (auto-generated from docstrings + handwritten guide)
├── /cli/                      (per-subcommand pages — port of docs/cli/)
│   ├── /passport-run
│   ├── /passport-verify
│   ├── /audit-export
│   └── ...
├── /api/                      (OpenAPI rendered — see §4.3)
├── /examples/
│   ├── /typescript-agent
│   ├── /python-agent
│   ├── /uniswap-agent
│   └── /langchain
├── /integrations/             (LangChain, AutoGen, ElizaOS, CrewAI, LlamaIndex)
└── /reference/
    ├── /errors                (every domain error code, sortable)
    ├── /schemas               (every JSON Schema, with examples)
    └── /security              (port of SECURITY_NOTES.md)
```

### 4.3 OpenAPI rendering

Two options:

| Option | Pros | Cons |
|---|---|---|
| **Stoplight Elements** (web component) | Pretty, industry-standard | ~120 KB JS; requires `script-src 'unsafe-eval'` for some versions — CSP problem |
| **Redoc CLI build → static HTML** | 0 KB JS at view time, full static | No interactive "try it" |
| **Hand-rolled MDX** with code samples | Full CSP control, indexed by Pagefind | High maintenance |

Recommendation: **Redoc CLI static build**. Run at CI, output `apps/docs/public/api/index.html`, link from sidebar. We sacrifice the in-browser try-it (Phase 3 can revisit) but stay under JS budget + CSP.

### 4.4 Docs information architecture

Frank's standing rule 1 (`03-agents.md` line 263): every doc starts with audience + outcome. Starlight supports frontmatter — we encode this:

```yaml
---
title: APRP wire format
description: How agents send payment intents to SBO3L.
audience: agent developer
outcome: After this page, you can construct a valid APRP envelope and POST it.
prereqs:
  - quickstart completed
  - basic JSON familiarity
---
```

A custom Starlight component (`AudienceBadge.astro`) renders the audience + outcome as a styled banner above the body. Heidi's regression test for "every doc has audience + outcome" becomes a frontmatter validator (zod schema in `src/content/config.ts`) — fails CI if missing.

### 4.5 Search (Pagefind)

Built-in. Index excludes `/api/` (Redoc has its own search). One-click setup in `astro.config.mjs`:

```js
import starlight from '@astrojs/starlight';

export default defineConfig({
  integrations: [
    starlight({
      title: 'SBO3L Docs',
      pagefind: true,
      // ...
    }),
  ],
});
```

### 4.6 Acceptance-criteria mapping

| AC from CTI-3-3 | Satisfied by |
|---|---|
| `docs.sbo3l.dev` resolves | Vercel project + DNS subdomain (Dev 4) |
| Search works | Pagefind, built-in, zero-JS-on-load |
| Code blocks runnable as-shown | Frank's standing rule 2; Heidi gates this in PR review (paste-and-run) |
| Lighthouse perf > 90 | Static Starlight, no external CDNs, system fonts |

### 4.7 Risks

- **Starlight CSP friction.** Some Starlight components inline `<style>` — `style-src 'self' 'unsafe-inline'` already handles it (matches starter's `vercel.json`). No new CSP relaxation.
- **OpenAPI freshness.** The Redoc static build needs CI wiring. If we miss the wiring, `/api/` docs lag the daemon. Mitigation: CI step in same workflow as `cargo doc`, fails build if `openapi.yaml` newer than rendered HTML.

---

## 5. CTI-3-4 — Hosted preview

### 5.1 Stack: Next.js 15 (App Router)

Ticket says "Next.js or SvelteKit". I'm recommending Next.js 15 because:

- **GitHub OAuth via NextAuth (Auth.js)** is the most battle-tested stack for "GitHub login in 30 minutes." SvelteKit's auth story (Lucia, etc.) is fine but one more thing to learn for the team.
- **App Router RSC** lets us co-locate the SBO3L daemon proxy logic next to the dashboard UI — fewer moving parts than a SvelteKit + separate API server split.
- **Vercel-first DX** — same deploy story as marketing + docs, even though the actual SBO3L daemon will run on Fly.io / Railway (Grace's call).

But: the hosted **frontend** is on Vercel/Cloudflare; the **daemon** runs elsewhere (Grace owns that — see §5.5). Next.js calls the daemon over HTTPS with a session-bound auth header.

Rejected:
- **SvelteKit** — viable, smaller bundles, but auth + ecosystem maturity favours Next.js for the 24h budget.
- **Plain React + Vite SPA** — no SSR, worse SEO + first paint, more rolling-our-own.

### 5.2 Surface

```
app.sbo3l.dev/
├── /                          (landing — login CTA, marketing-of-the-app)
├── /login                     (GitHub OAuth handoff)
├── /dashboard                 (post-login home)
│   ├── recent-decisions       (table: timestamp, intent, decision, deny_code)
│   ├── audit-chain-summary    (length, last checkpoint, integrity status)
│   └── quota                  (free-tier limits)
├── /agents                    (list + create new agent — issues ENS subname via Durin)
│   └── /[agent-id]/
│       ├── decisions          (filtered by agent)
│       ├── policy             (view active policy)
│       └── capsules           (download per decision)
├── /trust-dns                 (T-3-5 viz; embedded but full-page route)
├── /audit                     (full audit log explorer with strict-verify button)
├── /capsules                  (capsule library + verify-in-browser)
└── /settings                  (API token management, KMS config — Phase 3)
```

### 5.3 Wireframe — `/dashboard`

```
┌─────────────────────────────────────────────────────────────────┐
│ [SBO3L]    Dashboard  Agents  Audit  Trust-DNS    avatar▾       │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Hi, @<github-handle>.                                          │
│                                                                 │
│  ┌──────────────────┐ ┌──────────────────┐ ┌─────────────────┐  │
│  │ Decisions today  │ │ Audit chain      │ │ Quota used      │  │
│  │      127         │ │   length: 4,219  │ │   12% of 1k/day │  │
│  │ allow 119  deny 8│ │ ✓ verified now   │ │                 │  │
│  └──────────────────┘ └──────────────────┘ └─────────────────┘  │
│                                                                 │
│  Recent decisions (live, SSE-streamed)                          │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ 14:02:41  research-01  swap WETH→USDC  ALLOW  capsule▾     │ │
│  │ 14:02:38  research-01  pay vendor      DENY   policy.bg... │ │
│  │ 14:02:33  trader-02    quote check     ALLOW  capsule▾     │ │
│  │ ...                                                        │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                 │
│  [ Verify any capsule → ]  [ Download my agent fleet ]          │
└─────────────────────────────────────────────────────────────────┘
```

### 5.4 Real-time agent feed

Two transports under consideration:

| Transport | Pros | Cons | Eve's rule 7 |
|---|---|---|---|
| **SSE** (`EventSource`) | Native browser, auto-reconnect, simple | One-way only, but we don't need bidirectional from browser | ✓ |
| **WebSocket** | Bidirectional, lower latency tail | More complex (heartbeats, reconnect logic) | ✓ |

Eve's standing rule 7: real-time updates via native EventSource or WebSocket; **no `fetch()` polling**. Both are allowed.

Recommendation: **SSE on the dashboard**, **WebSocket on `/trust-dns`** (T-3-5 needs higher event volume + low-latency edge animation). Server emits the same events; dashboard subscribes to SSE for human-readable rows; viz subscribes to WS for graph deltas. Same backend code path, two transports.

The SSE/WS endpoint needs adding to `crates/sbo3l-server/`. Tracked under T-3-5 Files (`crates/sbo3l-server/src/ws_events.rs`). Dev 1 owns the daemon-side; Dev 3 consumes.

### 5.5 Auth flow

```
Browser → Next.js (App Router action) → NextAuth GitHub OAuth →
  → on success: NextAuth issues JWT (HttpOnly cookie, SameSite=Lax) →
  → Browser → Next.js server actions → daemon proxy on Fly.io →
  → daemon recognises per-tenant SQLite path from JWT.sub
```

**Per-tenant SQLite isolation:** Grace's call on the deployment side. Frontend never sees other tenants' data because every server-action call to the daemon includes the user's JWT, and the daemon resolves `JWT.sub → /data/users/<sub>/sbo3l.db`.

**Sensitive-ops gate:** the dashboard NEVER lets a user reveal their daemon's signing key (it's KMS-managed by Grace's deploy anyway). KMS key rotation is a server-side admin job, not an in-app feature.

### 5.6 JS budget projection

Next.js App Router on Vercel ships ~80-110 KB framework JS (cached across pages). Adding our app code:

| Page | Page-specific JS | Notes |
|---|---|---|
| `/` (landing) | +5 KB | mostly nav |
| `/login` | +8 KB | NextAuth client |
| `/dashboard` | +25 KB | live SSE feed + table virtualisation |
| `/trust-dns` | +D3 (≈55 KB) + viz code (≈15 KB) | budget watch — see §6.3 |
| `/audit` | +20 KB | log explorer |

Worst case (`/trust-dns` post-hydration): ~110 KB framework + 70 KB viz = ~180 KB gzipped. Tight against 200 KB. Mitigations: dynamic-import the D3 chunk so only `/trust-dns` pays the cost; consider `d3-force` + `d3-selection` only (skip `d3` umbrella ~30 KB savings). See [§6.3](#63-d3-bundle-strategy).

### 5.7 Acceptance-criteria mapping

| AC from CTI-3-4 | Satisfied by |
|---|---|
| `app.sbo3l.dev` hosts free tier | Vercel + Fly.io daemon (Grace) |
| Login works | NextAuth GitHub OAuth |
| Per-user state isolated | JWT.sub → SQLite path on daemon side |
| Dashboard shows real-time agent activity | SSE feed from daemon WS endpoint |
| Production runbook in `docs/ops/runbook.md` | Grace owns; Dev 3 contributes UI section |
| OTel traces flowing | Grace |
| Daily backup + restore-test | Grace |

Most ACs are split with Grace (Dev 4). Dev 3's slice is the frontend + the SSE/WS consumer + the dashboard UX.

---

## 6. T-3-5 — Trust DNS visualization

### 6.1 Stack decision

Ticket files imply Vite + D3 (`apps/trust-dns-viz/package.json (Vite + D3.js)`). However, the visualization will also be embedded in `app.sbo3l.dev/trust-dns` (CTI-3-4) per ticket spec.

**Two viable shapes:**

| Shape | Where | Trade-off |
|---|---|---|
| **A. Standalone Vite app** at `apps/trust-dns-viz/`, iframe-embed in Next.js | Both standalone preview AND inside hosted-app | Two builds, iframe boundary is awkward, but cleanest for ENS-MC demo video that wants a full-screen capture |
| **B. React/Preact component** lives inside `apps/hosted-app/` | Inside hosted app only | One build, deeper integration with auth/feed; but ENS Most Creative demo loses the standalone zero-chrome version |

**Recommendation: A (Vite standalone) + thin Next.js wrapper.** Reasons:

1. ENS Most Creative demo video benefits massively from a zero-chrome full-screen capture URL (`viz.sbo3l.dev` or `app.sbo3l.dev/trust-dns?embed=1`).
2. Standalone Vite build is easier to optimise for 60fps + 100 agents stress test (no Next.js framework overhead).
3. Embedding via iframe is one extra line (`<iframe src="/viz/" allow="..." />`) and CSP-clean (same-origin iframe).
4. The repo already has the file map for option A in T-3-5.

### 6.2 Architecture

```
apps/trust-dns-viz/
├── vite.config.ts
├── index.html              (single page; mounts <div id="viz">)
├── src/
│   ├── main.ts             (entry)
│   ├── graph.ts            (d3-force layout + render loop)
│   ├── ws.ts               (WebSocket client; reconnects on close)
│   ├── nodes.ts            (agent node rendering — circle, label, ENS name)
│   ├── edges.ts            (attestation edge rendering — animated path)
│   ├── events.ts           (event-type → graph mutation mapping)
│   └── style.css           (imports @sbo3l/design-tokens)
└── public/
    └── og-trust-dns.png    (preview image for social cards)
```

### 6.3 D3 bundle strategy

Don't import `d3` umbrella (~95 KB minified). Import only what we use:

```ts
import { forceSimulation, forceLink, forceManyBody, forceCenter } from 'd3-force';
import { select } from 'd3-selection';
import { drag } from 'd3-drag';
import { zoom } from 'd3-zoom';
```

Estimated bundle: ~30-35 KB gzipped (d3-force is the heavy one). Plus ~15 KB our code = ~50 KB total page weight. Comfortably under the 200 KB budget even when iframed inside Next.js.

### 6.4 Event protocol (WS payload, draft)

Server emits one of these per agent event. Spec lives in `crates/sbo3l-server/src/ws_events.rs` (T-3-5 file list). Dev 3 designs the payload shape; Dev 1 implements emission.

```ts
type VizEvent =
  | { kind: 'agent.discovered'; agent_id: string; ens_name: string; pubkey_b58: string; ts_ms: number }
  | { kind: 'attestation.signed'; from: string; to: string; attestation_id: string; ts_ms: number }
  | { kind: 'decision.made'; agent_id: string; decision: 'allow' | 'deny'; deny_code?: string; ts_ms: number }
  | { kind: 'audit.checkpoint'; agent_id: string; chain_length: number; root_hash: string; ts_ms: number };
```

Mapping to graph mutations:

| Event | Graph effect |
|---|---|
| `agent.discovered` | new node appears, fades in over 400ms |
| `attestation.signed` | edge appears between `from`-`to`, signed-badge animates along edge once |
| `decision.made` | node pulses green (allow) or red (deny); deny shows tooltip with `deny_code` |
| `audit.checkpoint` | node ring updates with current chain length; hover shows root hash |

### 6.5 60fps stress test plan

T-3-5 AC: 60fps with 100 agents. D3 force simulation is fine up to a few hundred nodes if we:

1. Use `simulation.alpha(0.1)` after initial layout — keep simulation cool.
2. Re-render via `requestAnimationFrame`, batch DOM mutations.
3. Use SVG for low node counts (< 200), switch to Canvas if 100-agent stress shows GC stutter.

A small bench harness (`apps/trust-dns-viz/bench.html` — not shipped) drives 100 events/sec via `setInterval`, watches Chrome perf devtools. Heidi runs this as part of T-3-5 QA.

### 6.6 Acceptance-criteria mapping

| AC from T-3-5 | Satisfied by |
|---|---|
| Renders 5 agents with edges | Force-directed layout with seed events on connect |
| WS updates < 1s latency | Direct WS, no polling, render in next animation frame |
| 60fps with 100 agents | Bundle strategy + Canvas fallback + alpha cooling |
| Mobile responsive | SVG viewBox + touch event mapping for drag/zoom |
| Lighthouse perf > 90 | Vite static build, lazy-load fonts (none — system stack), no external resources |
| Demo video centerpiece | Zero-chrome embed URL (`?embed=1`) for clean capture |

---

## 7. T-3-6 — Trust DNS essay

### 7.1 Surface

Lives at `docs.sbo3l.dev/concepts/trust-dns` (Starlight content collection). Frank authors in MDX — code blocks runnable, diagrams via Mermaid.

### 7.2 Skeleton (1500 words)

```md
---
title: Trust DNS — why ENS is the agent identity layer
audience: agent platform engineers, ENS community
outcome: You'll understand sbo3l:* records and the cross-agent verification protocol.
---

## Why this exists                              [~150 words]
## What "trust DNS" means                       [~250 words]
## The sbo3l:* record namespace                 [~300 words]
   - Reference: docs/spec/ens-text-records.md
## Cross-agent verification protocol            [~400 words]
   - Code: tests/test_cross_agent_verify.rs:1-120
## Failure modes                                [~200 words]
## Future work — CCIP-Read, ERC-8004           [~200 words]
```

Heidi gate: every code block is paste-and-runnable; every claim has a code reference (file:line) or test name.

### 7.3 Mirror to marketing blog

ENS-MC-A2 manifesto + T-3-6 essay both mirror to `sbo3l.dev/blog/`. Content collection lives once (in marketing repo), Starlight pulls via shared MDX import or simple file copy at build time. **Decision deferred to ENS-MC-A2 ticket** — for T-3-6 alone, single-source-of-truth at `docs.sbo3l.dev`, with marketing blog stub linking out.

---

## 8. ENS-MC-A1 — Trust DNS visual zine

### 8.1 Surface

Astro page at `apps/marketing/src/pages/trust-dns-story.astro`. Interactive zine — scroll-driven reveal, no autoplay, accessible (every panel has alt text).

### 8.2 Wireframe

```
┌─────────────────────────────────────────────┐
│ NAV                                         │
├─────────────────────────────────────────────┤
│                                             │
│   PANEL 1 (full viewport)                   │
│   "An agent wakes up.                       │
│    It has no name. It has no peers."        │
│   [zine illustration page 1]                │
│                                             │
│   ↓ scroll                                  │
│                                             │
│   PANEL 2 …                                 │
│                                             │
│   [8-12 panels total]                       │
│                                             │
│   FINAL PANEL                               │
│   "Read the spec → /docs/concepts/trust-dns"│
│   [ Download as PDF ]                       │
└─────────────────────────────────────────────┘
```

### 8.3 Tech notes

- **Scroll-driven**: CSS `position: sticky` + IntersectionObserver for panel transitions; no third-party scroll library.
- **Motion**: respect `prefers-reduced-motion`. If reduced, panels are static, no transitions.
- **Format**: panels are SVG (artist deliverable in vector — small, scalable, sharp on retina, ~200 KB total for full zine vs ~2 MB if PNG).
- **PDF download**: artist provides print-ready PDF; we serve from `public/trust-dns-zine/zine.pdf`.
- **Lighthouse**: lazy-load all panel images below the fold; the AC requires Lighthouse > 90 despite richer media.

### 8.4 Demo-video tie-in

ENS-MC-A1 AC: "Demo video opening sequence uses zine motion frames." Coordinate with Daniel — export 4-5 zine panels as a 5-second motion sequence (Lottie or hand-coded keyframe SVG) for the video opener.

---

## 9. Shared package — `@sbo3l/design-tokens`

To avoid token drift across four sites, propose a tiny internal package:

```
packages/design-tokens/
├── package.json    (name: "@sbo3l/design-tokens", private: true)
├── tokens.css      (the :root + [data-theme="light"] block from §2)
├── tokens.ts       (TS export of same tokens for JS consumers)
└── README.md
```

All four apps depend on it (workspace protocol via pnpm). Editing a token edits one file; all four sites pick up on next build. No risk of `--accent` drifting between marketing and docs.

Build cost: ~5 minutes setup. Maintenance: zero, except when we want to change a token (and then we want exactly this package).

---

## 10. Open questions for Daniel

Each is genuinely ambiguous; resolving them before CTI-3-2 starts saves rework.

1. **Q1: WASM verifier on `/proof`?** The starter today links out to GitHub Pages capsule download. CTI-3-2 says "capsule download + verifier instructions" (no live verifier mandated). Building a WASM verifier is ~6-10h additional work outside the 12h CTI-3-2 budget. **Default: link out, no live verifier.** Confirm or request escalation to a separate Phase 3 ticket.

2. **Q2: Live numbers strip — build-time or commit-back?** §3.5 proposes deploy-time injection (no git churn). Alternative: nightly GitHub Action commits `stats.json`. Daniel's call on whether bot commits are acceptable on main (vs deploy-only).

3. **Q3: Pricing-page tone — placeholder vs honest "no pricing yet"?** §3.3 sketches a 3-card placeholder. `01-identity.md` voice rule: "honest over slick." Alternative: `/pricing` is one paragraph saying "SBO3L is open-source. Hosted tier coming. Email if you want a pilot." which is more honest. **Default: paragraph form.** The 3-card sketch is in this doc only as a fallback if Daniel prefers a richer page.

4. **Q4: Light-theme on docs site — opt-in or auto via `prefers-color-scheme`?** §2 proposes manual toggle. Frank's audience reads at all hours; auto-switching might be smoother. **Default: auto via prefers-color-scheme + manual toggle override stored in localStorage.** Confirm storage approach is acceptable (no consent banner needed for non-tracking localStorage).

5. **Q5: Subdomain DNS approach.** `sbo3l.dev`, `docs.sbo3l.dev`, `app.sbo3l.dev` — all on Vercel? Or split (Vercel for marketing+docs, Fly.io for app)? §5.5 assumes split. Confirm with Grace before CTI-3-4 starts.

6. **Q6: Naming on apex domain redirect.** `www.sbo3l.dev` → `sbo3l.dev`, or no `www` at all? Modern convention is no `www`. **Default: redirect `www.` → apex.**

7. **Q7: Standalone viz subdomain.** §6.1 mentions option of `viz.sbo3l.dev` for zero-chrome embed. Or stay at `app.sbo3l.dev/trust-dns?embed=1`. **Default: query-param embed mode, no extra subdomain** — saves DNS + cert work.

---

## 11. Heidi-facing testability

Per Heidi's standing rule 4 (`03-agents.md` line 357): test plans must be pasteable. For each future ticket, the test plan will include:

- **CTI-3-2:** `pnpm --filter marketing build && pnpm --filter marketing preview`; Lighthouse via `npx lighthouse https://localhost:4321` reports > 90 on perf/a11y/best-practices.
- **CTI-3-3:** same, plus `pnpm --filter docs lint:frontmatter` (validates audience+outcome on every page).
- **CTI-3-4:** Playwright e2e — login flow + dashboard renders + SSE feed delivers an event within 2s of triggering it server-side.
- **T-3-5:** Playwright + bench harness — viz renders 5 agents from seed feed, then `bash demo-scripts/agent-fleet-stress.sh` drives 100 events; FPS measured via `performance.now()` snapshot in test, asserted ≥ 55fps p50.

Each ticket's PR will include the literal command set. Heidi runs verbatim.

---

## 12. Effort recheck against ticket budgets

| Ticket | Ticket budget | Design-doc estimate | Notes |
|---|---|---|---|
| CTI-3-2 | 12h | ~12h | Tight; Trust-DNS teaser + live numbers add ~2h, offset by reusing starter content |
| CTI-3-3 | 16h | ~16-18h | OpenAPI Redoc wiring is a wildcard; if it slips, drop `/api/` to Phase 3 follow-up |
| CTI-3-4 | 24h | ~24h frontend; +~12h Grace backend | Split owner already in ticket |
| T-3-5 | 12h | ~10-14h | Stress-test tuning is the variance; D3 + WS plumbing is well-scoped |
| **Total Dev 3** | **64h** | **~62-70h** | Within Phase 2 capacity (Dev 3 does no Phase 1 tickets) |

Phase 3 amplifiers (T-3-6 4h, ENS-MC-A1 10h, ENS-MC-A2 16h) are additional and unlock later.

---

## 13. Sequencing recommendation

When CTI-3-1 lands (Daniel buys `sbo3l.dev`):

```
Day 1   →  Stand up @sbo3l/design-tokens package (1h)
Day 1-3 →  CTI-3-2 marketing site (12h)        ─┐
Day 3-7 →  CTI-3-3 docs site     (16h)         ─┤  parallelisable; both static, no daemon dep
Day 7-12→  CTI-3-4 hosted app    (24h frontend)─┘  starts after F-1 (auth) + F-7 (Docker) merged
Day 12-15→ T-3-5 trust-dns viz   (12h)             needs T-3-3 + T-3-4 + CTI-3-4
```

Marketing + docs unblocked immediately on CTI-3-1; hosted app + viz wait for daemon-side dependencies.

---

## 14. What this doc is NOT

- Not a final visual-design comp. Eve does the polish in CTI-3-2 PR; this doc is wireframe-grade.
- Not a substitute for the actual tickets. Acceptance criteria still come from `06-phase-2.md`. This doc helps us not start cold.
- Not a commitment to every recommendation. Daniel's responses to §10 may shift any of these.

---

**Review notes for Daniel:** §10 is the request list. If you're short on time, just answering Q1, Q3, Q5 unblocks CTI-3-2 + CTI-3-3 entirely. Q2/Q4/Q6/Q7 are conventions we can decide in PR if not now.
