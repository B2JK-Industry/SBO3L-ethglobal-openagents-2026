# SDK install — final inventory (round 12 snapshot)

Generated 2026-05-02 from `scripts/sdk-install-verify.sh` (PR #239).

## Live registry status: 4/16

| Package | Ecosystem | Version | Live |
|---|---|---|---|
| `sbo3l-langchain` | PyPI | 1.2.0 | ✅ |
| `sbo3l-crewai` | PyPI | 1.2.0 | ✅ |
| `sbo3l-llamaindex` | PyPI | 1.2.0 | ✅ |
| `sbo3l-sdk` | PyPI | 1.2.0 | ✅ |
| `sbo3l-langgraph` | PyPI | 1.2.0 | ❌ trusted publisher missing |
| `sbo3l-agno` | PyPI | 1.2.0 | ❌ trusted publisher missing |
| `sbo3l-pydantic-ai` | PyPI | 1.2.0 | ❌ trusted publisher missing |
| `@sbo3l/sdk` | npm | 1.2.0 | ❌ NPM_TOKEN missing |
| `@sbo3l/langchain` | npm | 1.2.0 | ❌ NPM_TOKEN missing |
| `@sbo3l/autogen` | npm | 1.2.0 | ❌ NPM_TOKEN missing |
| `@sbo3l/elizaos` | npm | 1.2.0 | ❌ NPM_TOKEN missing |
| `@sbo3l/vercel-ai` | npm | 1.2.0 | ❌ NPM_TOKEN missing |
| `@sbo3l/openai-assistants` | npm | 1.2.0 | ❌ NPM_TOKEN missing |
| `@sbo3l/anthropic` | npm | 1.2.0 | ❌ NPM_TOKEN missing |
| `@sbo3l/anthropic-computer-use` | npm | 1.2.0 | ❌ NPM_TOKEN missing |
| `@sbo3l/mastra` | npm | 1.2.0 | ❌ NPM_TOKEN missing |
| `@sbo3l/vellum` | npm | 1.2.0 | ❌ NPM_TOKEN missing |
| `@sbo3l/langflow` | npm | 1.2.0 | ❌ NPM_TOKEN missing |
| `@sbo3l/inngest` | npm | 1.2.0 | ❌ NPM_TOKEN missing |
| `@sbo3l/marketplace` | npm | 1.2.0 | ❌ NPM_TOKEN missing |

(20 packages total — the matrix tracks 16 of these; `@sbo3l/anthropic-computer-use`, `@sbo3l/langflow`, `@sbo3l/inngest`, `@sbo3l/marketplace` are post-matrix additions and need to be added to `scripts/sdk-install-verify.sh`'s MATRIX block in a follow-up.)

## Recovery path

Per `docs/release/v1.2.0-recovery-runbook.md` (PR #228):

1. **Daniel provisions `NPM_TOKEN` repo secret** → unblocks 12 npm publishes
2. **Daniel adds PyPI trusted publishers** for `sbo3l-langgraph`, `sbo3l-agno`, `sbo3l-pydantic-ai` → unblocks 3 PyPI publishes
3. Re-fire the 15 failed publishes via `gh workflow run integrations-publish.yml --ref <tag>` (PR #225 added `workflow_dispatch`)
4. Re-run `SKIP_DOCKER=1 ./scripts/sdk-install-verify.sh` to confirm 16/16 (or 20/20 once matrix is extended)

Estimated total: 15 minutes of Daniel-side setup + ~2 minutes of CI per package.

## What the live PyPI packages prove

The 4 live PyPI packages exercise:
- `sbo3l-sdk` — the core async + sync clients
- `sbo3l-langchain` — LangChain Python tool descriptor
- `sbo3l-crewai` — CrewAI tool descriptor + Pydantic→dict coerce helper
- `sbo3l-llamaindex` — LlamaIndex tool descriptor

Together they cover: install resolves, top-level imports work, the SDK's strict-mode Pydantic types parse cleanly, and the framework adapter wrappers deliver their descriptor objects. **The wire format is real today**; only the npm distribution chain is gated on the token provisioning.

## Cross-references

- PR #239 — install verification matrix (this script)
- PR #228 — v1.2.0 recovery runbook
- PR #225 — `workflow_dispatch` for manual publish re-fires
