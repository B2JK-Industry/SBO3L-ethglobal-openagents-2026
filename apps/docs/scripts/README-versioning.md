# Versioned docs

`docs.sbo3l.dev` serves the latest docs at the apex (`/`) and a snapshot of every previous tagged release at `/<tag-id>/` — see [`apps/docs/src/data/versions.json`](../src/data/versions.json) for the registry.

## Layout served on Vercel

```
sbo3l-docs.vercel.app/                  → latest (built from current main)
sbo3l-docs.vercel.app/v1.0.0/           → tagged v1.0.0 snapshot
sbo3l-docs.vercel.app/v1.0.1/           → tagged v1.0.1 snapshot
sbo3l-docs.vercel.app/v1.2.0/           → tagged v1.2.0 snapshot
```

Add a new version by:

1. Tagging a release in git (`git tag v1.3.0 && git push --tags`).
2. Editing `src/data/versions.json` to add `{ "id": "v1.3.0", "tag": "v1.3.0", ... }`.
3. Re-running `npm run build:versioned`.

## Build

```bash
cd apps/docs
npm run build:versioned   # apex + every tagged snapshot
# Output: apps/docs/dist/                 (latest)
#         apps/docs/dist/v1.0.0/          (snapshot)
#         apps/docs/dist/v1.0.1/
#         apps/docs/dist/v1.2.0/
```

`scripts/build-versions.sh`:

1. Builds the apex from the current working tree (`ASTRO_BASE_PATH=""`).
2. For each entry in `versions.json` with a non-null `tag`, creates a temporary `git worktree` at that tag, runs `npm install` + `npm run build` with `ASTRO_BASE_PATH="/<id>"`, copies `dist/` into `apps/docs/dist/<id>/`.

`astro.config.mjs` reads `ASTRO_BASE_PATH` and threads it into Astro's `base` config so every link, asset path, and Pagefind search index is rooted correctly per version.

## VersionSelector

`src/components/VersionSelector.astro` reads `versions.json`, detects the current version from the URL prefix, and renders a `<details>`/`<ul>` dropdown that routes to the equivalent doc on a different version (`/v1.0.0/concepts/aprp` ↔ `/v1.0.1/concepts/aprp`).

Mount in the Starlight sidebar via the override slot — see Starlight's [`Sidebar.astro` override](https://starlight.astro.build/guides/overriding-components/) (separate ticket; current scope ships the component + script + version registry).

## Search

Pagefind is currently the search backend (already wired in CTI-3-3 prep). Each version gets its own Pagefind index because the build runs separately per version. Cross-version search is intentionally absent — searching the `latest` URL only finds latest content; switch versions and re-search.

Algolia DocSearch upgrade (more featureful search across versions, fuzzy matching) is its own ticket — needs an API key Daniel applies for, and Algolia indexes a single canonical URL per result so the per-version model needs configuration.

## CI

`.github/workflows/build-versioned-docs.yml` runs `npm run build:versioned` on every `v*` tag push and on manual `workflow_dispatch`. Steps:

1. **Detect tag** — from `github.ref` on tag push, or `git describe --tags` on manual dispatch.
2. **Register tag in versions.json** — `scripts/register-version.mjs <tag>` appends an entry just below `"latest"` (idempotent — no-op if tag already present). Bot-commits back to `main`.
3. **Build composite dist** — produces `apps/docs/dist/` with apex + every snapshot.
4. **Upload artifact** — composite dist as a 14-day-retention artifact.
5. **Deploy to Vercel** (tag push only, gated `environment: docs-prod`) — `npx vercel deploy dist --prebuilt --prod`. Requires `VERCEL_TOKEN`, `VERCEL_ORG_ID`, `VERCEL_DOCS_PROJECT_ID` secrets. Skips with a clear log line if `VERCEL_TOKEN` is unset.

Cutting a release end-to-end:

```bash
git tag v1.3.0
git push --tags
# CI fires → registers v1.3.0 → builds → deploys → docs.sbo3l.dev/v1.3.0/ live
```
