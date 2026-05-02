# `sbo3l-playground-api` — deploy runbook

Tier-3 of the SBO3L playground. Real Rust decision engine running
inside Vercel Functions (Node 24 LTS, wasm32-wasi). Real signed
receipts, real audit chain in Vercel Postgres, real on-chain
anchors to Sepolia AnchorRegistry.

This runbook is **for Daniel**. Run once at hackathon submission
time; the routes auto-activate when env vars become present.

## Prerequisites

- Vercel Pro plan (already active)
- The repo cloned locally
- `vercel` CLI installed: `pnpm dlx vercel --version`

## Phase 1 — Vercel project + data plane (~10 min)

### Step 1: Link the project

```sh
cd apps/sbo3l-playground-api
pnpm dlx vercel link
# When prompted: create new project named "sbo3l-playground-api"
# Custom domain (later): api.sbo3l.dev
```

This writes `.vercel/project.json` (gitignored). Don't commit it.

### Step 2: Provision Vercel Postgres

```sh
pnpm dlx vercel postgres create sbo3l-playground-db
```

This auto-injects `POSTGRES_URL` (and the read-only flavours) as
env vars on the project. No manual env-add needed.

After provisioning, apply the schema (one-shot):

```sh
psql "$POSTGRES_URL" < lib/migrations/V001_init.sql
```

(That migration file is a TODO — see "Phase 3" below.)

### Step 3: Provision Vercel KV

```sh
pnpm dlx vercel kv create sbo3l-playground-kv
```

Auto-injects `KV_REST_API_URL` + `KV_REST_API_TOKEN`.

### Step 4: Provision Vercel Blob

```sh
pnpm dlx vercel blob store create sbo3l-playground-blob
```

Auto-injects `BLOB_READ_WRITE_TOKEN`.

### Step 5: Generate Ed25519 signing key

```sh
openssl genpkey -algorithm ed25519 -out signing.pem
pnpm dlx vercel env add SBO3L_PLAYGROUND_SIGNING_KEY production
# Paste the contents of signing.pem (entire PEM including BEGIN/END lines)
rm signing.pem  # don't keep on disk
```

This is the per-deploy daemon signing key. Capsules from this Tier
3 deployment are signed with it; the `verifier_pubkey` field of
each capsule points to the matching public key (computed once at
boot from the private key, exposed via `/api/v1/healthz`).

## Phase 2 — Verify deploy (~2 min)

```sh
pnpm dlx vercel deploy --prod
```

Then probe healthz:

```sh
curl https://sbo3l-playground-api.vercel.app/api/v1/healthz
```

Expected before any of Phase 1 finishes:

```json
{
  "status": "skeleton",
  "env": { "has_postgres": false, "has_kv": false, "has_blob": false, "has_signing_key": false },
  "note": "Skeleton mode — provision ... per DEPLOY.md to activate."
}
```

After Phase 1 finishes:

```json
{
  "status": "ok",
  "env": { "has_postgres": true, "has_kv": true, "has_blob": true, "has_signing_key": true }
}
```

## Phase 3 — Wire route handlers (next round)

The route stubs in `app/api/v1/*` currently return `"status":
"skeleton"` placeholders. Each has TODO comments pointing at the
specific lib function it needs to wire:

| Route | TODO source |
|---|---|
| `POST /api/v1/decide` | `lib/wasm-loader.ts` (Rust → wasm32-wasi build), `lib/db.ts`, `lib/blob.ts`, signer |
| `GET /api/v1/capsule/[id]` | `lib/blob.ts` `fetchCapsule` |
| `GET /api/v1/audit/chain` | `lib/db.ts` `queryAuditChain` |
| `POST /api/v1/decide` (rate limit) | `lib/kv.ts` `checkRateLimit` middleware |

The `wasm-loader` is the load-bearing piece — the real `sbo3l-core`
crate doesn't yet ship a `wasm32-wasi` build target with C-ABI
exports for `decide_aprp` / `build_capsule`. That's a separate
task on the Rust side. Once landed:

```sh
# Inside crates/sbo3l-core/
cargo build --release --target wasm32-wasi --features playground-api
cp target/wasm32-wasi/release/sbo3l_core.wasm \
   ../../apps/sbo3l-playground-api/lib/sbo3l-core.wasm
```

Then drop the `import { decideAprp } from "./sbo3l-core.wasm"` into
`app/api/v1/decide/route.ts` and remove the placeholder return.

## Phase 4 — Anchor cron (optional, +5 min)

Every 6 hours, publish the audit-chain root to the Sepolia
AnchorRegistry contract at `0x4C302ba8...E8f4Ac` (already deployed).

Add the deployer wallet's private key:

```sh
pnpm dlx vercel env add SBO3L_DEPLOYER_PRIVATE_KEY production
# Paste 0x-prefixed hex private key (65 chars total)
```

The cron itself lives in `.github/workflows/playground-anchor-publish.yml`
(separate PR — needs the daemon API up first to compute the root).

## Phase 5 — Domain + monitoring

### Custom domain

```sh
pnpm dlx vercel domains add api.sbo3l.dev
```

Update `apps/marketing/src/pages/playground/live.astro` to point at
`https://api.sbo3l.dev` instead of `*.vercel.app`. (Currently uses
the vercel.app URL as fallback — works the moment the project
deploys.)

### Vercel Analytics + Speed Insights

Both flip on via project Settings → Analytics. Free on Pro plan.

### Sentry

```sh
pnpm dlx vercel env add SENTRY_DSN production
pnpm add @sentry/nextjs --filter @sbo3l/playground-api
```

Sentry's Next.js wizard handles instrumentation; out of scope for
the skeleton.

## Cost ceiling

Vercel Pro covers Functions / Postgres / KV / Blob within
generous limits. Anchor cron uses ~24K gas every 6h on Sepolia
(test ETH, free). No surprise bills as long as rate-limit (10
req/min/IP) holds.

## Common gotchas

| Symptom | Fix |
|---|---|
| `healthz` says skeleton after Phase 1 | Redeploy: env vars only inject on next build. `vercel deploy --prod`. |
| `decide` returns 502 with "wasm-loader skeleton" | Phase 3 not done — Rust → wasm32-wasi build pending. |
| Rate-limit fires on every request | KV not provisioned; `lib/kv.ts` returns `allowed: true` in skeleton mode. After provisioning, the bucket initializes. |
| Anchor cron silent | Phase 4 deployer key not set; cron skips with a workflow log. |
