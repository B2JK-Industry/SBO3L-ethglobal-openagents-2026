# `@sbo3l/playground-api`

Tier-3 of the SBO3L playground. Vercel-hosted real daemon that
returns signed receipts and persists the audit chain to Postgres.

**Status:** skeleton. Routes return `"status": "skeleton"` placeholders
until provisioned per [`DEPLOY.md`](./DEPLOY.md).

## Routes (planned)

| Route | Method | What it does |
|---|---|---|
| `/api/v1/healthz` | GET | env-presence probe (works in skeleton mode) |
| `/api/v1/decide` | POST | run real `sbo3l-core` decision, return signed capsule |
| `/api/v1/capsule/[id]` | GET | fetch stored capsule (7-day TTL) |
| `/api/v1/audit/chain` | GET | latest 100 audit events + on-chain anchor link |

## Architecture

- **Next.js 15 App Router** on Vercel Functions (Fluid Compute, Node 24 LTS)
- **`sbo3l-core` Rust crate** compiled to wasm32-wasi, loaded once per warm container
- **Vercel Postgres** for `audit_events`, `seen_nonces`, `idempotency_keys`
- **Vercel KV** for per-IP rate limiting (10 req/min)
- **Vercel Blob** for capsule storage (7-day TTL)
- **Sepolia AnchorRegistry** at `0x4C302ba8…E8f4Ac` for on-chain audit-chain anchors (6h cron)

## Why a separate Vercel project (not part of `sbo3l-marketing`)

- **Cold start budget:** Functions on the marketing project would inflate the static-site build with WASM. Splitting keeps `sbo3l-marketing.vercel.app` static + sub-second.
- **Independent rate limit:** Tier 3 carries a real Stripe-billable cost surface eventually; isolating it makes per-IP throttling cleaner.
- **Domain hygiene:** `api.sbo3l.dev` (this project) vs `sbo3l.dev` (marketing) is the canonical web pattern.

## What's NOT in this project

- The Tier-2 mock playground UI (lives in `apps/marketing/src/pages/playground.astro`)
- The Tier-3 page UI (lives in `apps/marketing/src/pages/playground/live.astro` — calls into this API)
- The on-chain anchor cron workflow (separate `.github/workflows/playground-anchor-publish.yml`)
- The `wasm32-wasi` build of `sbo3l-core` — pending in the Rust workspace

## Local development

```sh
pnpm --filter @sbo3l/playground-api install
pnpm --filter @sbo3l/playground-api dev
# → http://localhost:3100/api/v1/healthz
```

## Test plan

- `pnpm --filter @sbo3l/playground-api typecheck` passes
- `pnpm --filter @sbo3l/playground-api build` produces a clean Next.js build
- All 4 route stubs return placeholder JSON with HTTP 200 (or 400 on bad input)
- `vercel.json` validates against the `vercel.json` schema
- `DEPLOY.md` is paste-runnable end-to-end without ambiguity
