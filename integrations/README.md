# SBO3L framework integrations

Drop-in adapters that wrap `@sbo3l/sdk` (TS) or `sbo3l-sdk` (Py) for the dominant LLM frameworks. One per directory; each ships independently to npm or PyPI via `.github/workflows/integrations-publish.yml`.

| Package | Path | Registry | Tag |
|---|---|---|---|
| `@sbo3l/langchain` | `integrations/langchain-typescript/` | npm | `langchain-ts-v*` |
| `@sbo3l/autogen` | `integrations/autogen/` | npm | `autogen-v*` |
| `@sbo3l/elizaos` | `integrations/elizaos/` | npm | `elizaos-v*` |
| `@sbo3l/vercel-ai` | `sdks/typescript/integrations/vercel-ai/` | npm | `vercel-ai-v*` |
| `sbo3l-langchain` | `integrations/langchain-python/` | PyPI | `langchain-py-v*` |
| `sbo3l-crewai` | `integrations/crewai/` | PyPI | `crewai-py-v*` |
| `sbo3l-llamaindex` | `integrations/llamaindex/` | PyPI | `llamaindex-py-v*` |
| `sbo3l-langgraph` | `sdks/python/integrations/langgraph/` | PyPI | `langgraph-py-v*` |

## Per-package release flow

1. Bump the package's `version` field (`package.json` for npm, `pyproject.toml` for PyPI).
2. Open + merge a PR with the bump.
3. Tag locally and push:
   ```bash
   git tag langchain-ts-v1.1.0
   git push origin langchain-ts-v1.1.0
   ```
4. `.github/workflows/integrations-publish.yml` fires the matching matrix entry: verifies the tag version equals the file version, builds, publishes (npm with provenance, PyPI via Trusted Publishing).

Tag-version mismatch fails the workflow before publishing — the version in the file is the source of truth.

## Publish credentials Daniel must provision

- **npm** — `NPM_TOKEN` repo secret + `@sbo3l` scope (one-time setup).
- **PyPI** — Trusted Publishing config in repo environment `pypi-<id>` (one per integration: `pypi-langchain-py`, `pypi-crewai-py`, `pypi-llamaindex-py`, `pypi-langgraph-py`).

`@sbo3l/sdk` (npm) and `sbo3l-sdk` (PyPI) have their own publish workflows — `sdk-typescript.yml` + `sdk-python.yml`.

## Build-test on every PR

The `npm-build-test` and `pypi-build-test` matrix jobs run on every PR that touches an integration dir or the workflow file. Each integration's tests + lint + (build wheel|build dist) run in isolation; failures don't block sibling integrations (`fail-fast: false`).
