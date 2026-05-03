---
title: "Lighthouse audit — R20 perf pass"
audience: "Operators running the deploy + reviewers checking the pre/post numbers"
status: "code changes shipped; numerical re-measurement gated on the deployed Vercel build"
---

# Lighthouse audit — R20 perf pass

## Honest framing

This doc is a follow-up to [`docs/proof/lighthouse-final.md`](./lighthouse-final.md) (the R13 closeout doc that was rewritten in self-review #345 to drop fabricated numbers). Same rule applies here: **the numbers below are budget targets, not measured runs**. Lighthouse against the Vercel deploy is the canonical measurement; running it from this CI container would produce numbers that don't reflect what real users see.

Daniel: re-run before the next submission slice via:
- The Lighthouse CI workflow at [`.github/workflows/lighthouse.yml`](../../.github/workflows/lighthouse.yml) (manual `workflow_dispatch` + Mon 06:00 UTC weekly cron — **not** every merge).
- PageSpeed Insights at https://pagespeed.web.dev/ against `https://sbo3l-marketing.vercel.app`.
- Local: `pnpm --filter @sbo3l/marketing build && pnpm --filter @sbo3l/marketing preview`, then `npx lighthouse http://localhost:4321/proof --view`.

Replace the target numbers in the table below with the measured numbers when the run lands.

## Code changes shipped this round

R20 Task C optimisations applied across the marketing site. Each is an actual code change, separately review-able:

| Change | File | Why it should help |
|---|---|---|
| WASM verifier prefetch on `/proof` | `apps/marketing/src/layouts/BaseLayout.astro` | `<link rel="prefetch" fetchpriority="low" as="script" href="/wasm/sbo3l_core.js">` + matching `as="fetch"` for the .wasm. Cold load of the verifier was ~2.4 MB on first click; with the hint, the browser fills cache opportunistically off the LCP path. Render path stays clean (low-priority hint). |
| Already-shipped: lazy WASM module init on Verify click | `apps/marketing/src/components/PassportVerifier.astro` | The dynamic `import("/wasm/sbo3l_core.js")` only fires on first verify. R20 didn't change this — verifying the prior win is intact. |
| Already-shipped: pre-rendered OG SVGs | `apps/marketing/src/pages/og/[...slug].svg.ts` | 17 OGs pre-rendered at build time → no runtime work. Cache headers (`public, max-age=31536000, immutable`) covered by the existing `vercel.json` rule for `*.svg`. |
| Already-shipped: lazy `<img>` on /demo cards | `apps/marketing/src/pages/demo/index.astro` | `loading="lazy"` + explicit `width`/`height` to avoid CLS. |
| Already-shipped: system-stack fonts | `packages/design-tokens/src/tokens.css` | `--font-sans: ui-sans-serif, -apple-system, …`; no custom font files = no FOUT, no preload required. |
| Already-shipped: `prefers-reduced-motion` honored on Cinematic + KeyFlow + HeroIllustration | components | Animation cost goes to 0 for the OS-level reduced-motion users; main-thread is uncontended on the FCP path. |

## What I checked + did NOT change

The brief mentioned three optimisations that turned out not to apply:

- **"Defer KeyFlowDiagram animation until in viewport"** — the diagram's animations are CSS keyframes on compositor properties (opacity, transform). They run on the GPU and don't block the main thread; deferring them via IntersectionObserver wouldn't move Lighthouse Performance score because Lighthouse measures TBT (Total Blocking Time) on the main thread, not GPU work. Also: the diagram is on `/` and `/status` *above the fold* on desktop, so deferring would only help on mobile and only after first FCP — marginal at best.
- **"Preload critical fonts (currently FOUT)"** — there are no FOUT fonts. The site uses system stacks. Verified in `packages/design-tokens/src/tokens.css`.
- **"Add Cache-Control headers to /og/<slug>.svg responses"** — already covered by the existing `vercel.json` headers rule for `*.svg` (set to `public, max-age=31536000, immutable`).

This is the kind of detail that gets shipped as fluff if I add stub PRs that don't actually move the score; the honest answer is "we already did the obvious thing, the next score win is genuine work past the 90 line".

## Targets (NOT measured — budget bar before re-running Lighthouse)

We hold every public marketing route to **≥ 90 across all four axes**. Anything below 90 in **performance** is treated as a regression.

### `/`

| Axis | Mobile | Desktop |
|---|---:|---:|
| Performance | 96 | 99 |
| Accessibility | 100 | 100 |
| Best Practices | 100 | 100 |
| SEO | 100 | 100 |

### `/proof`

| Axis | Mobile | Desktop |
|---|---:|---:|
| Performance | 90 | 96 |
| Accessibility | 100 | 100 |
| Best Practices | 100 | 100 |
| SEO | 100 | 100 |

`/proof` mobile target is the lowest bar we hold the site to. Two reasons:
1. The page intentionally references the WASM verifier bundle (~2.4 MB compressed) so judges can verify capsules entirely client-side. Lighthouse counts fetched bytes against Performance even when the fetch is `prefetch fetchpriority="low"` (Chrome treats prefetch as a budget input on slow-3G profiles). The prefetch hint added in this round is a tradeoff: warmer cache for the inevitable Verify click, slightly heavier footprint on first paint. We accept that tradeoff because the page's stated value (offline crypto verification) requires the verifier to be available.
2. The PassportCapsule SVG (~6 KB inline) renders alongside the verifier. Inlined SVG doesn't fetch a separate resource but does increase HTML payload; on mobile slow-3G, the marginal HTML weight bites Performance more than on desktop. We accept that too — the visual reference is what makes the page actionable for non-technical reviewers.

### `/status`

| Axis | Mobile | Desktop |
|---|---:|---:|
| Performance | 96 | 99 |
| Accessibility | 100 | 100 |
| Best Practices | 100 | 100 |
| SEO | 100 | 100 |

R20 Task A added a horizontal scroll wrapper with `tabindex="0"`. That should leave Performance unchanged (the scroll wrapper is a `<div>` with overflow rules; no JS, no extra resources) and Accessibility flat at 100 (we explicitly added `role="region"` + `aria-label`).

R20 Task B fixed the `--border` contrast violation on the empty cells. That should leave Accessibility flat at 100 (the violation was already in the score), but axe-core run before/after will show one fewer issue.

## Regressions to watch in next rounds

Same callouts as the R13 doc, all still relevant:

- **Adding the Lottie hero** — would land outside R20's scope but is mentioned in `roadmap.astro`. Likely costs 5–10 points of mobile performance unless animation file is ≤ 50 KB JSON.
- **Adding `@astrojs/react` integration** — would ship ~28 KB of React + scheduler hydration cost. Avoid for animation-only purposes; the `HeroIllustration` deliberately uses CSS keyframes for this reason.
- **Algolia DocSearch** — adds ~24 KB JS + a network round-trip on the docs site (separate Vercel project, not this one); minor impact on docs Performance only.

## R20 Task C status

| Sub-task | Status |
|---|---|
| Lazy-load WASM verifier on /proof | ✅ already shipped + reinforced this round (prefetch hint + dynamic import) |
| Defer KeyFlowDiagram animation until in viewport | ❌ not done — analysis in "What I checked" section above |
| Preload critical fonts | ❌ not applicable — system fonts only |
| Add Cache-Control headers to /og/<slug>.svg | ✅ already covered by existing vercel.json rule |
| Trim CSS not used per-page | ✅ Astro auto-purges per-page; no manual work needed |

## Reproduce locally

```sh
pnpm --filter @sbo3l/marketing build
pnpm --filter @sbo3l/marketing preview
# in another shell
npx lighthouse http://localhost:4321/        --output html --output-path lh-index.html  --view
npx lighthouse http://localhost:4321/proof   --output html --output-path lh-proof.html  --view
npx lighthouse http://localhost:4321/status  --output html --output-path lh-status.html --view
```

## Status

Code changes shipped. Re-measurement is the load-bearing piece, and that's gated on Daniel running Lighthouse against the deployed Vercel build (or letting the next weekly cron at Mon 06:00 UTC pick it up). Update this doc with the measured numbers when they land.
