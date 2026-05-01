# SBO3L Docs Site (Astro Starlight)

Documentation site, deployed to `sbo3l-docs.vercel.app` (future home: `docs.sbo3l.dev`). Astro Starlight 0.30+, Pagefind built-in search, no external CDNs, strict CSP.

## Local preview

```bash
cd apps/docs
npm install
npm run dev      # http://localhost:4321
```

## Production build

```bash
npm run build    # output: apps/docs/dist/
npm run preview  # serve dist/ locally
```

## Deploy on Vercel

`apps/docs/vercel.json` configures Vercel:

- `framework: astro` — Vercel auto-detects build settings.
- `buildCommand: npm run build` — runs `astro build`.
- `outputDirectory: dist`.
- Strict CSP carried over from marketing site.

Link this directory as a project's **Root Directory** → `apps/docs` in the Vercel dashboard. Push to `main` triggers production deploy.

## Frontmatter contract (Frank's standing rule)

Every doc MUST declare its audience and outcome at the top of the file. Enforced by the schema in `src/content.config.ts`; the build fails CI if any doc omits them.

```mdx
---
title: APRP wire format
description: How agents send payment intents to SBO3L.
audience: agent developer
outcome: After this page, you can construct a valid APRP envelope and POST it.
---
```

Optional fields: `prereqs` (array of strings), Starlight's standard fields (`template`, `hero`, `lastUpdated`, etc.).

## Files (current scope — prep slice)

```
apps/docs/
├── astro.config.mjs           # Starlight integration + sidebar tree
├── package.json               # @astrojs/starlight + @sbo3l/design-tokens
├── tsconfig.json
├── vercel.json                # CSP + cache
├── src/
│   ├── content.config.ts      # extends docsSchema with required audience + outcome
│   ├── content/docs/
│   │   ├── index.mdx          # splash landing
│   │   ├── quickstart.mdx     # stub
│   │   ├── concepts/index.mdx # stub
│   │   ├── sdks/index.mdx     # stub
│   │   ├── cli/index.mdx      # stub
│   │   ├── api/index.mdx      # stub
│   │   ├── examples/index.mdx # stub
│   │   ├── integrations/index.mdx # stub
│   │   └── reference/index.mdx    # stub
│   └── styles/custom.css      # maps Starlight theme tokens to @sbo3l/design-tokens
└── public/
```

Sidebar entries pointing at unwritten pages render with a `soon` badge until the content port lands in CTI-3-3 main.

## Roadmap

This PR is the **prep slice** of CTI-3-3. The follow-up adds:

- QUICKSTART.md content ported into `/quickstart`
- Concept guides (APRP, audit-log, capsule v2, policy, budget, sponsor-adapters, trust-dns)
- SDK references (TypeScript + Python — generated from JSDoc / docstrings + handwritten guides)
- CLI per-subcommand pages (port of `docs/cli/*`)
- OpenAPI rendered via Redoc CLI static build at `/api`
- Error codes reference, schema reference, security notes (port of `SECURITY_NOTES.md`)
- T-3-6 Trust DNS essay at `/concepts/trust-dns` (1500 words)

## Search

Pagefind, built-in. Zero-JS-on-load — indexed at build time, lazy-loads search worker on user input. Same-origin, CSP-clean. No external service.

## What this site is NOT

- Not the marketing site (CTI-3-2, deployed at `sbo3l-marketing.vercel.app`)
- Not the hosted app (CTI-3-4, deploys at `sbo3l-app.vercel.app`)
- Not the API server (the daemon runs locally or on Fly.io; this site documents it)
