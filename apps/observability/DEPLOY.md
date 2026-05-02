# Deploy `apps/observability`

The dashboard is a static Astro site (~100 KB JS gzipped after recharts tree-shake). Any CDN works.

## Vercel (recommended, ~2 min)

```bash
cd apps/observability
npx vercel link        # one-time, picks the project
npx vercel deploy      # preview deploy
npx vercel deploy --prod
```

Or via the Vercel dashboard:

1. **Import Git Repository** → pick this repo
2. **Root Directory** → `apps/observability`
3. **Framework Preset** → Astro (auto-detected)
4. **Build Command** → `npm run build`
5. **Output Directory** → `dist`

That's it. Vercel auto-rebuilds on every push to `main` that touches `apps/observability/**`.

## Live data

By default the dashboard renders mock data (60-minute synthetic snapshot at 8-15 RPS). To point it at a running daemon:

```
https://your-vercel-url.vercel.app/?endpoint=https://your-daemon.example.com
```

The dashboard fetches `<endpoint>/v1/admin/metrics` on hydration. CORS: the daemon must allow the dashboard's origin (the daemon's CORS handling is documented in `crates/sbo3l-server/README.md`).

## Local dev

```bash
cd apps/observability
npm install
npm run dev              # http://localhost:4321
npm test                 # 16 vitest passing
npm run build            # produces ./dist/
npm run preview          # serve ./dist/ locally
```

## Why Astro

- **Static output** — no server runtime, deploys to any CDN
- **React islands** — chart panels are React (Recharts), the rest is pure Astro for fast first paint
- **Build once, point anywhere** — same artifact serves mock + any daemon endpoint via `?endpoint=`

## Wire format

The dashboard reads `MetricsSnapshot` from `/v1/admin/metrics`. Wire shape is documented in `src/lib/metrics.ts`; reference data in `src/data/mock-metrics.ts`. **The `/v1/admin/metrics` endpoint isn't shipped on `sbo3l-server` yet** — the dashboard ships ahead so the wire format is locked before the server-side implementation lands.

## Out of scope

- Per-framework drilldowns — fast follow once `/v1/admin/metrics` returns the framework label dimension
- Alerting / thresholds — operators run their own Prometheus + Alertmanager against the same metrics
- Real-time streaming — the dashboard polls on hydration; live websocket streaming is a future ticket
