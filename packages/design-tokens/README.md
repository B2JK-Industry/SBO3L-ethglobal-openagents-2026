# @sbo3l/design-tokens

Shared design tokens for the SBO3L brand surface. Two parallel exports:

- **CSS variables** — `import "@sbo3l/design-tokens/css"` registers `:root` variables. CSS-only consumers (Astro pages, Starlight themes) use this.
- **Typed TS object** — `import { tokens, darkTokens, lightTokens } from "@sbo3l/design-tokens"`. JS consumers (D3 force-graph fills, computed inline styles) use this.

## Why this package exists

Four sites consume the same brand surface:

| Site | Stack | Surface |
|---|---|---|
| `sbo3l.dev` | Astro 5 | marketing |
| `docs.sbo3l.dev` | Astro Starlight | docs |
| `app.sbo3l.dev` | Next.js 15 | hosted preview |
| `app.sbo3l.dev/trust-dns` | Vite + D3 | trust-dns visualisation |

Without a shared package the accent colour drifts between sites in days. With this package, one edit fans out everywhere on next build.

## Tokens included

- **Colour:** `bg`, `fg`, `muted`, `accent`, `codeBg`, `border` (dark + light theme variants).
- **Layout:** `max` (920px prose width), `maxApp` (1280px dashboard width).
- **Type:** `sans` + `mono` system stacks (no external font CDN per CSP), 5-step modular scale, two line-height defaults.
- **Spacing:** 4-px grid radii (`sm`/`md`/`lg`).

See [`src/tokens.css`](./src/tokens.css) and [`src/tokens.ts`](./src/tokens.ts) for canonical values. The two files MUST stay in sync — a future test will assert this; for now, edit in pairs.

## Theme switching

`tokens.css` ships three themes:

1. `:root` defaults (dark).
2. `[data-theme="light"]` opt-in via DOM attribute.
3. `@media (prefers-color-scheme: light)` honour OS preference unless `data-theme="dark"` overrides.

Manual override: set `document.documentElement.dataset.theme = "light" | "dark"` and persist to `localStorage`. (Implemented in marketing + docs sites; see [Q4 in design doc](../../docs/design/phase-2-frontend.md#10-open-questions-for-daniel).)

## Status

**Scaffold only.** Not yet consumed by any site (Phase 2 unlocks once `sbo3l.dev` is purchased — CTI-3-1). Contents reflect the design doc at [`docs/design/phase-2-frontend.md`](../../docs/design/phase-2-frontend.md) §2.

## Build

```bash
pnpm --filter @sbo3l/design-tokens build
# or, locally:
cd packages/design-tokens && npx tsc
```

Output lands in `dist/` (gitignored).

## License

MIT (will track repo root license once that exists).
