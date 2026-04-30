# SBO3L Marketing Site (Astro 5)

Static marketing site, deployed to `sbo3l-marketing.vercel.app` (future home: `sbo3l.dev` once Daniel buys the domain). Astro 5, no SSR, no external CDNs, strict CSP.

## Local preview

```bash
cd apps/marketing
npm install      # pulls @sbo3l/design-tokens from packages/design-tokens
npm run dev      # http://localhost:4321
```

## Production build

```bash
npm run build    # output: apps/marketing/dist/
npm run preview  # serve dist/ locally
```

## Deploy on Vercel

`apps/marketing/vercel.json` configures Vercel:

- `framework: astro` — Vercel auto-detects build settings.
- `buildCommand: npm run build` — runs `astro build`.
- `outputDirectory: dist` — Astro's default static output.
- Strict CSP + cache headers + `/proof` redirect (until WASM verifier sub-ticket lands).

In the Vercel dashboard, link this directory as a project's **Root Directory** → `apps/marketing`. Push to `main` triggers production deploy.

## Files (current scope)

```
apps/marketing/
├── astro.config.mjs           # static, output: dist/
├── package.json               # depends on @sbo3l/design-tokens (file: link)
├── tsconfig.json
├── vercel.json                # CSP + cache + redirect
├── src/
│   ├── layouts/BaseLayout.astro
│   ├── components/{Nav,Footer,NumberStrip}.astro
│   ├── pages/index.astro
│   └── styles/global.css      # imports @sbo3l/design-tokens/css
└── public/
```

## Roadmap

This PR is the **prep slice** of CTI-3-2. The follow-up adds:

- Live integration evidence (ENS / Uniswap / KH) panel.
- Adversarial-block list panel.
- Architecture diagram (`ArchDiagram.astro`).
- Reproduce-yourself code block.
- `/features`, `/proof`, `/trust-dns-story` routes.
- Blog content collection (T-3-6 essay, ENS-MC-A2 manifesto land here).
- WASM-compiled verifier embedded on `/proof` (separate sub-ticket; see [issue #92](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/issues/92)).

## Security

Strict CSP enforced via `vercel.json`:

- `default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'`
- No external font, CSS, script, image, or analytics CDNs.
- System font stacks only.
- `X-Frame-Options: DENY`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: strict-origin-when-cross-origin`.
- `Permissions-Policy: camera=(), microphone=(), geolocation=()`.

## What this site is NOT

- Not the proof page (redirects to `/proof` → GitHub Pages capsule download until WASM verifier sub-ticket lands).
- Not the docs site (CTI-3-3, deploys at `sbo3l-docs.vercel.app`).
- Not the hosted app (CTI-3-4, deploys at `sbo3l-app.vercel.app`).
