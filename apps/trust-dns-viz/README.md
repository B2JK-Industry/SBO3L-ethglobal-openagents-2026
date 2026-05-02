# SBO3L Trust DNS Visualization

Real-time force-directed graph of agents discovering and verifying each other via ENS. Standalone Vite + d3-force; embeds in `sbo3l-app.vercel.app/trust-dns` via iframe with `?embed=1`.

## Local preview

```bash
cd apps/trust-dns-viz
npm install
npm run dev      # http://localhost:4322 (mock event source by default)
```

URL params: `?embed=1` (hide chrome for iframe), `?ws=wss://...` (real events from Dev 1's `ws_events.rs` once it lands; defaults to mock when absent), `?mock=1` (force mock for debugging).

## Production build + deploy

`npm run build` → `dist/`. `vercel.json`: framework vite; CSP allows WS to `wss://sbo3l-app.vercel.app`; `frame-ancestors` allows iframe-embed by `sbo3l-app.vercel.app` and `app.sbo3l.dev`.

## Event protocol

`src/events.ts` — see `VizEvent` type. Server side (Dev 1, T-3-5 backend slice) emits the same shapes; the mock generator in source.ts simulates them.

## Status

Prep + standalone preview shipped. Real-WebSocket consumer is wired but defaults to mock until Dev 1's `crates/sbo3l-server/src/ws_events.rs` ships. Once backend lands, iframe-embed in `sbo3l-app.vercel.app/trust-dns` follows in a small CTI-3-4 follow-up.

Roadmap items (canvas fallback, stress harness, tooltips, reduced-motion respect) tracked under T-3-5 main.
