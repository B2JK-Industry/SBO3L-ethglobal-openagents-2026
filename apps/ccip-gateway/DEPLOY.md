# Deploying `@sbo3l/ccip-gateway` to Vercel

**Audience:** Daniel (or any operator authorised to run Vercel + GitHub
admin operations).

**Outcome:** in 15 minutes the gateway is live at
`https://sbo3l-ccip.vercel.app` and any push to `main` that touches
`apps/ccip-gateway/**` auto-deploys.

## One-time project setup

### 1. Create the Vercel project

```bash
cd apps/ccip-gateway
npx vercel link
# answer:
#   - Set up and link?         Y
#   - Which scope?             B2JK-Industry (or personal)
#   - Link to existing project? N
#   - Project name?            sbo3l-ccip
#   - Directory?               ./
#   - Framework auto-detected: Next.js
```

After this, `apps/ccip-gateway/.vercel/project.json` exists with
`projectId` and `orgId` — note these for step 3.

`apps/ccip-gateway/.vercel/` is already in the workspace `.gitignore`
(repo root), so the local link is ephemeral.

### 2. Set the runtime env var on Vercel

```bash
# Generate a fresh secp256k1 key (never reused with any wallet that
# holds funds — this is a *signing* key for read-side gateway responses):
node -e "console.log('0x' + require('crypto').randomBytes(32).toString('hex'))" \
  | tr -d '\n' | pbcopy   # copy to clipboard

vercel env add GATEWAY_PRIVATE_KEY production
# paste from clipboard
vercel env add GATEWAY_PRIVATE_KEY preview
# paste the SAME value (preview env mirrors prod for E2E parity)
vercel env add GATEWAY_PRIVATE_KEY development
# paste the SAME value (or a separate dev key — your call)
```

The address corresponding to this key is what gets baked into the
on-chain OffchainResolver's `signer` storage when T-4-1 deploys.

### 3. Add GitHub Actions secrets

Repository → Settings → Secrets and variables → Actions → "New
repository secret":

| Secret               | Value                                                           |
|----------------------|-----------------------------------------------------------------|
| `VERCEL_TOKEN`       | from `https://vercel.com/account/tokens` (scope: full account, no expiry preferred) |
| `VERCEL_ORG_ID`      | from `apps/ccip-gateway/.vercel/project.json` `orgId`           |
| `VERCEL_PROJECT_ID`  | from `apps/ccip-gateway/.vercel/project.json` `projectId`       |

`GATEWAY_PRIVATE_KEY` does **not** go in GitHub secrets — only Vercel.
The deploy workflow does not need to read the key; only the running
runtime does.

### 4. Trigger the first deploy

```bash
git commit --allow-empty -m "ci: bootstrap ccip-gateway deploy"
git push origin main
```

Watch the workflow at `https://github.com/B2JK-Industry/SBO3L-…/actions/workflows/ccip-gateway.yml`.

When it goes green, smoke:

```bash
curl -sS https://sbo3l-ccip.vercel.app/ | head -5
# expect: <!DOCTYPE html>... (the landing page from src/app/page.tsx)

curl -sS -o /dev/null -w "%{http_code}\n" \
  https://sbo3l-ccip.vercel.app/api/0x0000000000000000000000000000000000000000/0x00.json
# expect: 501 (pre-T-4-1 stub)
```

## CI workflows

### `.github/workflows/ccip-gateway.yml`

- `typecheck` job: `npm run typecheck` + `npm run build` on every PR
  and main push. Catches TS errors before deploy.
- `deploy-preview` job: on PR — Vercel preview URL, comment posted on
  the PR with the URL + smoke command.
- `deploy-production` job: on push to `main` — Vercel production
  deploy. Promotes to `https://sbo3l-ccip.vercel.app`.

### `.github/workflows/ccip-gateway-uptime.yml`

- Runs every 30 minutes against the production URL.
- GETs landing page (expect 200), GETs the stub API endpoint
  (currently expects 501; flip to 404 once T-4-1 ships record
  lookup).
- Posts a `::warning::` annotation on any non-2xx/3xx response so
  Heidi's daily regression sweep catches outages.

## Custom domain (post-`sbo3l.dev` unfreeze)

Once Daniel acquires `sbo3l.dev`, attach `ccip.sbo3l.dev` to this
project:

```bash
vercel domains add ccip.sbo3l.dev sbo3l-ccip
```

Then update T-4-1's OffchainResolver `urls` array to the custom
domain. The Vercel project keeps responding on both URLs during the
transition.

## Rotating the gateway signing key

```bash
# 1. Generate a new key
NEW=$(node -e "console.log('0x' + require('crypto').randomBytes(32).toString('hex'))")
# 2. Update Vercel env (overwrites)
vercel env rm GATEWAY_PRIVATE_KEY production -y
vercel env add GATEWAY_PRIVATE_KEY production
# paste NEW
# 3. Trigger redeploy
vercel --prod
# 4. Update OffchainResolver on-chain with the new address
sbo3l agent update-resolver-signer \
  --network mainnet \
  --new-signer 0x<address derived from NEW>
# (sbo3l agent update-resolver-signer ships in a follow-up; for now,
#  call the contract's setSigner method directly via cast/foundry.)
```

## Cost budget

| Resource                     | Cost              |
|------------------------------|-------------------|
| Vercel Hobby plan            | free              |
| Function invocations         | free up to 100k/mo (uptime probe + judges' resolution traffic ≪ this) |
| Bandwidth                    | free up to 100 GB/mo |
| Custom domain                | free (DNS only)   |
| `GATEWAY_PRIVATE_KEY`        | $0 (no funds held)|

Total recurring: **$0** while we stay within Vercel Hobby limits.
Upgrade to Pro ($20/mo) only if function invocations exceed 100k/mo —
unlikely during the hackathon submission window.

## Troubleshooting

- **Build fails with "Cannot find module 'next'"** — `npm install`
  was skipped. The workflow caches by `package-lock.json`; if the
  lockfile is missing, the cache key breaks. Run `npm install` once
  locally and commit `package-lock.json`.
- **Deploy succeeds but landing page 404s** — Vercel's "Output
  Directory" setting is wrong. Verify the project root is
  `apps/ccip-gateway`, not the repo root, in the Vercel project's
  Build & Deployment settings.
- **`vercel pull` fails with "project not found"** — `VERCEL_ORG_ID`
  / `VERCEL_PROJECT_ID` mismatch. Re-fetch from
  `apps/ccip-gateway/.vercel/project.json` after a fresh `vercel
  link`.
- **Uptime workflow alerts on 200 instead of 501** — T-4-1 has shipped
  and the stub is gone. Update the assertion in
  `.github/workflows/ccip-gateway-uptime.yml` to expect 404 for
  unknown records.
