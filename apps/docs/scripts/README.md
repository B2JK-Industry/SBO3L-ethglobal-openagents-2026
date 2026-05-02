# SDK reference auto-generation

Two scripts pull the latest published `@sbo3l/*` and `sbo3l-*` packages and run typedoc / sphinx against them. Output lands in `apps/docs/public/sdk-ref/{typescript,python}/<package>/` as static HTML, served alongside the Starlight site.

## Local regeneration

```bash
# TypeScript (npm packages)
bash apps/docs/scripts/gen-sdk-ts-refs.sh

# Python (PyPI packages)
bash apps/docs/scripts/gen-sdk-py-refs.sh
```

Both scripts work in temp dirs and clean up after themselves; the only persistent change is `apps/docs/public/sdk-ref/`. Re-run after every release of `@sbo3l/sdk@X.Y.Z` or `sbo3l-sdk==X.Y.Z`.

## CI

`.github/workflows/sdk-refs.yml` invokes both scripts on:

- Manual `workflow_dispatch`.
- Daily cron (catches publishes from external mirrors / forks).
- Repository dispatch event `sdk-published` (npm/PyPI publish flows fire this — see the publish workflow in each SDK repo).

The workflow commits the regenerated `apps/docs/public/sdk-ref/` back to `main` via a bot token. PRs are not generated — the regenerated content is a deterministic function of the published packages, and the wrapper MDX pages don't change.

## Wrappers

Each SDK has a thin Starlight wrapper page at `apps/docs/src/content/docs/reference/sdk-typescript/<short>.mdx` (or `sdk-python/<short>.mdx`) declaring audience + outcome per Frank's rule and linking out to the auto-gen HTML. The wrapper survives schema validation; the auto-gen HTML lives outside the content collection so it's not constrained by the schema.

## Package lists

| Language | Package | Wrapper page |
|---|---|---|
| TS | `@sbo3l/sdk` | `/reference/sdk-typescript/sdk` |
| TS | `@sbo3l/langchain` | `/reference/sdk-typescript/langchain` |
| TS | `@sbo3l/autogen` | `/reference/sdk-typescript/autogen` |
| TS | `@sbo3l/vercel-ai` | `/reference/sdk-typescript/vercel-ai` |
| Py | `sbo3l-sdk` | `/reference/sdk-python/sdk` |
| Py | `sbo3l-langchain` | `/reference/sdk-python/langchain` |
| Py | `sbo3l-crewai` | `/reference/sdk-python/crewai` |
| Py | `sbo3l-llamaindex` | `/reference/sdk-python/llamaindex` |

Add a package: append it to `PACKAGES=(...)` in the relevant gen script, add a wrapper MDX, add a sidebar entry in `astro.config.mjs`. CI re-runs do the rest.
