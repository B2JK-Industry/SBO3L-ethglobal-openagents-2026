# Deploying the hosted app

`apps/hosted-app/` is the Next.js 15 hosted preview at `sbo3l-hosted-app.vercel.app` (future home: `app.sbo3l.dev`). CI deploys via `.github/workflows/hosted-app.yml` on every merge to `main`.

## Operator one-time setup (Daniel)

Run once; CI handles every subsequent deploy.

### 1. Create the Vercel project

```bash
cd apps/hosted-app
npx vercel link --yes --project sbo3l-hosted-app --scope <your-org-slug>
# Writes apps/hosted-app/.vercel/project.json with the org/project IDs.
cat .vercel/project.json
```

The `.vercel/` directory is already gitignored — it's a local artefact, not committed.

### 2. Add repo secrets

In **GitHub repo Settings → Secrets and variables → Actions**:

| Secret | Source |
|---|---|
| `VERCEL_TOKEN` | https://vercel.com/account/tokens (scope: full account) |
| `VERCEL_ORG_ID` | `cat apps/hosted-app/.vercel/project.json` (orgId field) |
| `VERCEL_HOSTED_APP_PROJECT_ID` | `cat apps/hosted-app/.vercel/project.json` (projectId field) |

Until these three secrets are set, the deploy step in `hosted-app.yml` skips with a warning; CI build + typecheck still run.

### 3. Set Vercel project env vars

In **Vercel project → Settings → Environment Variables → Production**:

| Variable | Value | Notes |
|---|---|---|
| `AUTH_SECRET` | `npx auth secret` output | NextAuth JWT signing key |
| `AUTH_URL` | `https://sbo3l-hosted-app.vercel.app` | (or future `https://app.sbo3l.dev` once DNS lands) |
| `AUTH_GITHUB_ID` | from https://github.com/settings/developers | Authorization callback URL: `${AUTH_URL}/api/auth/callback/github` |
| `AUTH_GITHUB_SECRET` | same source | |
| `AUTH_GOOGLE_ID` | optional — Google Cloud Console | enables "Continue with Google" |
| `AUTH_GOOGLE_SECRET` | optional | |
| `AUTH_APPLE_ID` | optional — Apple Services ID | enables "Continue with Apple"; secret is a JWT generated per Apple's flow |
| `AUTH_APPLE_SECRET` | optional | |
| `SBO3L_DAEMON_URL` | URL of the running daemon | `http://localhost:8080` works for local; production wants a publicly-reachable daemon (Fly.io/Railway/Render — Grace's slice) |
| `ADMIN_GITHUB_LOGINS` | comma-separated lowercase logins | grants `admin` role; e.g. `babjak-daniel` |
| `ADMIN_EMAILS` | comma-separated lowercase emails | same effect via email match |
| `OPERATOR_GITHUB_LOGINS` | optional | grants `operator` role |
| `OPERATOR_EMAILS` | optional | |

### 4. Configure GitHub Environment

In **GitHub repo Settings → Environments → New environment**:

- Name: `hosted-app-prod`
- Optional: protection rules (required reviewers before deploy can proceed) — recommended for production.

The workflow's `deploy` job declares `environment: hosted-app-prod` so the URL appears in the GitHub deployment surface and any approval rules fire.

## What CI does on every push to main

1. **`build-typecheck`** — installs deps, runs `npm run typecheck`, runs `npm run build` with placeholder env (verifies build graph compiles; auth deactivated at build time, activated at runtime by Vercel env).
2. **`deploy`** — only on `push` to `main`. `vercel pull` + `vercel build --prod` + `vercel deploy --prebuilt --prod`. Skips with a clear log line if `VERCEL_TOKEN` is unset.
3. **`smoke`** — only after a successful deploy. Curls `/`, `/login`, and `/dashboard`; expects 200 (200, 200, 307-or-200 respectively for the auth-gated route).

## Live URL

Once deployed, the canonical preview URL is `https://sbo3l-hosted-app.vercel.app`. Add it to `docs/submission/live-url-inventory.md` (already done in this PR).

## Troubleshooting

- **Smoke test fails with `Login: 500`.** The most common cause is a missing `AUTH_SECRET` env var on Vercel. NextAuth refuses to start without one.
- **Smoke test fails with `Dashboard: 200`** but you expected a redirect — the route loaded the public landing instead of the auth-protected page. Check that `middleware.ts` matcher includes `/dashboard/:path*` (it does) and that the deploy actually picked up the latest middleware.ts.
- **Daemon-status banner shows red on `/dashboard`.** `SBO3L_DAEMON_URL` is set but the daemon at that URL doesn't respond to `/v1/healthz` within 4 seconds. Either the daemon is down, the URL is wrong, or there's a network policy blocking the egress from Vercel.

## See also

- [`apps/hosted-app/README.md`](./README.md) — the app's local-dev story.
- [`.github/workflows/hosted-app.yml`](../../.github/workflows/hosted-app.yml) — the deploy workflow itself.
