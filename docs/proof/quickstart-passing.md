# Quickstart guides — static validation passing

Generated 2026-05-02 by `scripts/quickstart-validate.sh` (PR #260).

## Status: 5/5 ✅

| Guide | File | Result |
|---|---|---|
| `keeperhub-with-langchain` | `docs/quickstart/keeperhub-with-langchain.md` | ✅ ok |
| `keeperhub-with-openai-assistants` | `docs/quickstart/keeperhub-with-openai-assistants.md` | ✅ ok |
| `uniswap-with-vercel-ai` | `docs/quickstart/uniswap-with-vercel-ai.md` | ✅ ok |
| `uniswap-with-mastra` | `docs/quickstart/uniswap-with-mastra.md` | ✅ ok |
| `ens-with-anthropic` | `docs/quickstart/ens-with-anthropic.md` | ✅ ok |

## What passing means

The 7-check static validator (`scripts/quickstart-validate.sh`) confirms each guide:

1. Lists all expected packages in its `npm i` / `pip install` block
2. References every expected SDK export name in code (no drift between guide and shipped API)
3. Has all 12 APRP v1 required fields in its fixture
4. Contains **no** frozen `01HTAWX5K3R8YV9NQB7C6P2DG{M,N,P}` nonces (round-4 codex P1 regression net)
5. Contains **no** expired `2026-05-0?T10:31:00Z` expiries
6. Calls a dynamic-nonce helper (`crypto.randomUUID` in TS, `uuid.uuid4` in Py)
7. Uses the policy-allowlisted recipient `0x1111...1111` on `chain: base`

What this **doesn't** guarantee — runtime end-to-end against a live LLM. That requires:
- Live npm packages (NPM_TOKEN gap; see `docs/release/v1.2.0-recovery-runbook.md`)
- Model API keys (cost + secrets we don't run in CI)

The `sdk-install-matrix` workflow (PR #239) covers package liveness; Heidi runs LLM-driven smokes manually.

## Re-run

```bash
./scripts/quickstart-validate.sh                                       # local
gh workflow run quickstart-validation.yml --ref main                   # CI
```

Nightly cron at 04:30 UTC catches drift introduced by SDK refactors that don't themselves touch the docs.

## Workflow runs (last triggered today)

- Run 25251994543 (workflow_dispatch from this round) — see Actions tab
- Nightly cron will fire at 04:30 UTC and post results to the workflow's run log
