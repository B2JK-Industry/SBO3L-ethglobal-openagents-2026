# /demo step screenshots

Static screenshot assets for the `/demo` walkthrough pages. Captured by `apps/marketing/scripts/capture-demo-screenshots.mjs` (Playwright + Chromium).

## File layout

```
apps/marketing/public/demo/
├── step-1.svg        (placeholder until first capture; replaced by step-1.png)
├── step-1-mobile.svg
├── step-2.svg
├── step-2-mobile.svg
├── step-3.svg
├── step-3-mobile.svg
├── step-4.svg
└── step-4-mobile.svg
```

The committed `.svg` files are typed-out placeholders rendering a force-graph silhouette + "screenshot pending" caption. The capture script outputs **`.png`** (1440×900 desktop, 375×667 mobile). When real PNGs land, swap the `.svg` references in the three demo `.astro` files to `.png` (one-line change per file) and commit the new PNGs alongside.

## How they're used

The `/demo` landing page renders these as `<picture>`-element-driven cards (mobile variant via `(max-width: 640px)` source). Each step's `1-meet-the-agents.astro` (and `4-explore-the-trust-graph.astro`) embeds the desktop variant as a hero-card linking out to the live trust-dns viz.

Loading is lazy (`loading="lazy"` on every `<img>`); the screenshots are served as static PNGs from Vercel's edge cache, immutable per the `*.png` rule in `apps/marketing/vercel.json`.

## Regeneration — CI (preferred)

The fastest path is **GitHub Actions**:

1. Open the [Capture /demo screenshots workflow](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/actions/workflows/capture-demo-screenshots.yml).
2. Click **Run workflow** → optionally override `target_url` → leave `open_pr: true` ticked → **Run workflow**.
3. The workflow runs Playwright against the deployed marketing site, commits 8 PNGs, swaps `.svg` → `.png` in the 3 referencing files, opens an auto-merge PR.

No terminal access needed; Daniel triggers via the GitHub UI.

## Regeneration — local

```bash
cd apps/marketing
npm install --no-save playwright
npx playwright install chromium
npm run dev &                       # boot local server in another shell
node scripts/capture-demo-screenshots.mjs
```

Default base URL is `http://localhost:4321`; override via `DEMO_BASE_URL=https://sbo3l-marketing.vercel.app node scripts/...` to capture against the deployed site.

## First-run state

The 8 PNGs are committed to the repo so the site renders correctly even before anyone has run the capture script. If the page UI changes meaningfully, run the script and commit the regenerated PNGs.

## Why static PNGs, not iframe loads

The previous `/demo/1` and `/demo/4` pages embedded the live viz via iframe. That cost ~600 KB of JS + simulation startup time before judges saw anything. PNG screenshots render immediately, lazy-load below the fold, and click through to the live viz for anyone who wants the interactive experience. Lighthouse mobile score climbs from ~70 to ~95 with this swap.
