# /api page — OpenAPI + Redoc

The docs site renders the daemon's HTTP API at `https://sbo3l-docs.vercel.app/api` using [Redoc Standalone](https://github.com/Redocly/redoc) v2.1.5 against `/openapi.yaml`.

## Build flow

`npm run build` prebuild hook runs two scripts in sequence:

1. **`scripts/copy-openapi.sh`** copies `crates/sbo3l-server/openapi.yaml` into `apps/docs/public/openapi.yaml`. Spec lives in the daemon crate so it stays close to the source-of-truth implementation; docs site re-copies on every build so the served spec never drifts.
2. **`scripts/fetch-redoc.sh`** downloads the pinned Redoc Standalone bundle into `apps/docs/public/redoc/redoc.standalone.js`. Idempotent — skips if the version file matches the pin. ~700 KB; cached immutably by Vercel.

Both outputs are gitignored — the bundle gets pulled fresh on every CI build, and the OpenAPI spec is derivative, so neither is committed.

## CSP relaxation

Redoc Standalone uses `Function()` constructor for runtime spec parsing — that requires `script-src 'unsafe-eval'`. The `/api` route alone gets a relaxed CSP in `apps/docs/vercel.json`; every other route (concepts, CLI, reference, etc.) keeps the strict `default-src 'self'` policy.

The relaxation is narrow in scope (one route) and well-documented (this file). When/if Redoc ships an `'unsafe-eval'`-free build, swap to it.

## Local preview

```bash
cd apps/docs
npm install
npm run build       # runs prebuild → build
npm run preview
# open http://localhost:4321/api
```

## Updating the pinned Redoc version

Edit `REDOC_VERSION` in `apps/docs/scripts/fetch-redoc.sh`. Next build pulls the new bundle.

## Updating the OpenAPI spec

Edit `crates/sbo3l-server/openapi.yaml`. Next docs build picks it up. The spec is hand-written today; future ticket may switch to utoipa-extracted YAML built from the axum handlers themselves.

## Why not a Starlight content page

Astro Starlight's content collection enforces the `audience` + `outcome` frontmatter fields (Frank's rule, validated at build time). The Redoc render is a tool, not prose — declaring an outcome would be contrived. So `/api` lives as a top-level Astro page at `apps/docs/src/pages/api.astro`, outside the Starlight collection. The page still imports the design tokens for visual continuity.
