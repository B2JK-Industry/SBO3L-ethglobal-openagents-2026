# SBO3L Hosted App (Next.js 15)

Free-tier hosted SBO3L preview, deployed to `sbo3l-app.vercel.app` (future home: `app.sbo3l.dev`). Next.js 15 App Router, NextAuth v5 GitHub OAuth, no client-side data storage.

## Local preview

```bash
cd apps/hosted-app
cp .env.example .env.local
# Fill in AUTH_GITHUB_ID, AUTH_GITHUB_SECRET, AUTH_SECRET (npx auth secret)
npm install
npm run dev      # http://localhost:3000
```

## Production build

```bash
npm run build    # next build
npm start        # next start
```

## Deploy on Vercel

`apps/hosted-app/vercel.json` configures Vercel:

- `framework: nextjs` — Vercel auto-detects.
- Strict CSP relaxed for Next.js: `script-src 'self' 'unsafe-inline'` (RSC streaming injects inline scripts; CTI-3-4 main PR migrates to nonce-based CSP).
- `connect-src` includes `https://api.github.com` for NextAuth GitHub provider.
- `img-src` includes `https://avatars.githubusercontent.com` for user avatars.

Required Vercel project env vars:

| Key | Value |
|---|---|
| `AUTH_SECRET` | random 32-byte secret (`npx auth secret`) |
| `AUTH_URL` | `https://sbo3l-app.vercel.app` |
| `AUTH_GITHUB_ID` | from `https://github.com/settings/developers` |
| `AUTH_GITHUB_SECRET` | from `https://github.com/settings/developers` |

## Auth flow

1. Browser → `/login` → form posts to NextAuth → GitHub OAuth → callback → JWT cookie set.
2. Browser → `/dashboard` → `middleware.ts` checks JWT → render or redirect to `/login`.
3. Sign-out → form posts to `signOut` server action → cookie cleared → redirect home.

JWT-backed sessions (no DB in prep). Main PR adds a DB adapter once Postgres lands in Grace's Fly.io deploy.

## Files (current scope)

```
apps/hosted-app/
├── auth.ts                 # NextAuth v5 config (GitHub provider, JWT, login callback)
├── middleware.ts           # auth-protect /dashboard/*, /agents/*, /audit/*, /capsules/*
├── next.config.mjs         # strict mode, no x-powered-by header
├── package.json            # next@15, next-auth@5-beta, react@19
├── tsconfig.json
├── vercel.json             # CSP + framework: nextjs
├── .env.example            # AUTH_SECRET / AUTH_URL / AUTH_GITHUB_ID/SECRET / SBO3L_DAEMON_URL
├── app/
│   ├── layout.tsx          # root html
│   ├── page.tsx            # landing
│   ├── login/page.tsx      # GitHub sign-in form
│   ├── dashboard/page.tsx  # auth-protected; shows handle + 3 placeholder cards
│   ├── api/auth/[...nextauth]/route.ts  # NextAuth handlers
│   └── globals.css         # imports @sbo3l/design-tokens
└── README.md
```

## Roadmap (CTI-3-4 main PR)

- Live SSE feed of agent decisions (consumes daemon WS endpoint Dev 1 ships).
- Recent-decisions table with virtualized scrolling.
- `/agents` create + list (issues ENS subname via Durin once Ivan's path lands).
- `/audit` explorer with strict-verify button.
- `/capsules` library + verify-in-browser (uses #101 WASM verifier when available).
- Per-tenant SQLite isolation by JWT `sub` (Grace owns daemon-side path layout).
- OpenTelemetry traces (Grace).
- Migrate to nonce-based CSP — drop `'unsafe-inline'` from `script-src`.

## What this app is NOT

- Not the marketing site (CTI-3-2, `sbo3l-marketing.vercel.app`).
- Not the docs site (CTI-3-3, `sbo3l-docs.vercel.app`).
- Not the daemon — daemon runs on Fly.io / Railway; this UI calls it.
