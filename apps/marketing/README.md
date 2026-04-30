# SBO3L Marketing Site (Starter)

Static landing page for `sbo3l.dev` (or any host). Plain HTML + CSS, no JS, no external CDNs, no fetch.

## Local preview

```bash
# Any static server. Examples:
python3 -m http.server -d apps/marketing 8080
# or
npx serve apps/marketing
```

Open `http://localhost:8080`.

## Deploy on Vercel

This repo's root `vercel.json` configures Vercel to serve `apps/marketing/` as the static output directory. After connecting the repo:

1. Vercel auto-detects on push to `main`
2. Production deploy → assigned `*.vercel.app` URL
3. Custom domain (`sbo3l.dev`) added in Vercel dashboard → DNS A record → done

## Roadmap

This is the starter Eve replaces in Phase 2 ticket **CTI-3-2** with a full Astro/Next.js marketing site (richer features, blog, case studies, animations). See `docs/win-backlog/06-phase-2.md#cti-3-2`.

## Files

- `index.html` — single-page landing
- `style.css` — handcrafted CSS (no Tailwind, no preprocessor)
- `vercel.json` — security headers + cache rules
- `README.md` — this file

## Security headers

Set via `vercel.json`:
- `Content-Security-Policy: default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'` — no external resources
- `X-Content-Type-Options: nosniff`
- `X-Frame-Options: DENY`
- `Referrer-Policy: strict-origin-when-cross-origin`
- `Permissions-Policy: camera=(), microphone=(), geolocation=()`

## What this page is NOT

- Not the proof page (redirects to `/proof` → GitHub Pages capsule download)
- Not the docs site (CTI-3-3, deploys at `docs.sbo3l.dev`)
- Not the hosted app (CTI-3-4, deploys at `app.sbo3l.dev`)

This page is the front door at `sbo3l.dev` — pitch + numbers + evidence + links.
