# Dev 2 closeout — status report

**As of 2026-05-02 round 15.** All Dev 2 PRs accounted for, both merged and in-flight.

## Tally

| State | Count |
|---|---|
| **MERGED** | 35 |
| OPEN (will land via this PR or follow-on) | 1 (this PR) |
| CLOSED (superseded) | 1 (#277, dupe of #273) |
| **TOTAL** | **37** |

35/36 merge ratio = **97%** clean merge rate. The single CLOSED was an URGENT hotfix duplicate that landed via the wider omnibus PR #273 instead.

## All Dev 2 PRs (chronological newest first)

| PR | Round | Title | State |
|---|---|---|---|
| #319 | R14 | cross-protocol LIVE harness + Slack ChatOps bot (P7+P3) | MERGED |
| #314 | R14 | GitLab + CircleCI + Jenkins capsule-verify plugins (P6) | MERGED |
| #308 | R14 | 8 framework adapters (cohere/together/perplexity/replicate/modal/e2b/agentforce/copilot-studio) | MERGED |
| #286 | R13 | 4 adapters (letta/autogpt/babyagi/superagi) + sbo3l-verify GitHub Action | MERGED |
| #277 | R13 | URGENT package.json hotfix | CLOSED (dupe of #273) |
| #273 | R12 | cross-protocol killer demo + /v1/admin/metrics + 4 proof docs | MERGED |
| #260 | R11 | quickstart static drift detection (P5) | MERGED |
| #256 | R11 | sbo3l-marketplace CLI bin (P3) | MERGED |
| #252 | R10 | apps/observability dashboard (4 panels) | MERGED |
| #249 | R10 | @sbo3l/langflow + @sbo3l/inngest + sbo3l-pydantic-ai (T-1-15) | MERGED |
| #244 | R10 | @sbo3l/marketplace SDK (Phase 3.3) | MERGED |
| #239 | R10 | SDK install verification end-to-end (P1) | MERGED |
| #228 | R9 | v1.2.0 recovery runbook | MERGED |
| #225 | R9 | workflow_dispatch for publish re-runs | MERGED |
| #224 | R9 | bounty × framework quickstart guides (5) | MERGED |
| #220 | R8 | @sbo3l/anthropic-computer-use (T-1-13) | MERGED |
| #216 | R8 | Keep-a-Changelog for 5 net-new adapters | MERGED |
| #214 | R8 | @sbo3l/vellum + sbo3l-agno (T-1-12) | MERGED |
| #207 | R7 | @sbo3l/mastra (T-1-11) | MERGED |
| #199 | R7 | MCP HTTP transport + Claude Desktop/Cursor/Continue docs | MERGED |
| #195 | R7 | @sbo3l/anthropic (T-1-10) | MERGED |
| #193 | R7 | @sbo3l/openai-assistants (T-1-9) | MERGED |
| #179 | R5 | MEV guard policy module (Track 5) | MERGED |
| #166 | R5 | examples/uniswap-agent-{ts,py} (T-5-6) | MERGED |
| #165 | R5 | T-5-1 Uniswap full swap construction | MERGED |
| #155 | R4 | examples/multi-framework-agent (cross-framework killer) | MERGED |
| #154 | R4 | examples/langgraph research agent (8/8) | MERGED |
| #153 | R4 | examples/llamaindex research agent (7/8) | MERGED |
| #152 | R4 | examples/elizaos research agent (6/8) | MERGED |
| #151 | R4 | examples/autogen research agent (5/8) | MERGED |
| #148 | R3 | examples/langchain-py research agent (4/8) | MERGED |
| #145 | R3 | per-package release workflow + matrix (CI) | MERGED |
| #144 | R3 | examples/vercel-ai research agent (3/3) | MERGED |
| #143 | R3 | examples/crewai research agent (2/3) | MERGED |
| #137 | R3 | examples/langchain-ts research agent (1/3) | MERGED |
| #131 | R2 | sbo3l-langgraph PolicyGuardNode adapter (T-1-8) | MERGED |

## What this delivered

### 30 framework adapters (TypeScript + Python)

| TypeScript (npm) | Python (PyPI) |
|---|---|
| `@sbo3l/sdk` | `sbo3l-sdk` |
| `@sbo3l/langchain` ✅ live | `sbo3l-langchain` ✅ live |
| `@sbo3l/autogen` ✅ live | `sbo3l-crewai` ✅ live |
| `@sbo3l/elizaos` ✅ live | `sbo3l-llamaindex` ✅ live |
| `@sbo3l/vercel-ai` | `sbo3l-langgraph` ✅ live |
| `@sbo3l/openai-assistants` | `sbo3l-agno` |
| `@sbo3l/anthropic` | `sbo3l-pydantic-ai` |
| `@sbo3l/anthropic-computer-use` | |
| `@sbo3l/mastra` | |
| `@sbo3l/vellum` | |
| `@sbo3l/langflow` | |
| `@sbo3l/inngest` | |
| `@sbo3l/marketplace` | |
| `@sbo3l/letta` | |
| `@sbo3l/autogpt` | |
| `@sbo3l/babyagi` | |
| `@sbo3l/superagi` | |
| `@sbo3l/cohere-tools` | |
| `@sbo3l/together` | |
| `@sbo3l/perplexity` | |
| `@sbo3l/replicate` | |
| `@sbo3l/modal` | |
| `@sbo3l/e2b` | |
| `@sbo3l/agentforce` | |
| `@sbo3l/copilot-studio` | |

**Live state:** 8/30 packages live on registries. The other 22 are gated on Daniel's NPM_TOKEN provisioning + 2 PyPI trusted publishers + a 30-min workflow matrix extension. Wire format is correct on every live package — recovery path documented in PR #228.

### Apps + tooling

- `examples/cross-protocol-killer/` (PR #273) — 10-step single-agent demo walking ENS + 6 LLM frameworks + KH + Uniswap → capsule + 6-check verifier
- `examples/multi-framework-agent/` (PR #155) — LangChain → CrewAI → AutoGen unified audit chain
- 8 per-framework example research-agent demos (PRs #137-154)
- `apps/observability/` (PR #252) — Astro dashboard, 4 panels, mock+live mode
- `apps/chatops-{slack,discord,teams}/` (PR #319 + this PR) — code-only ChatOps bots
- `actions/sbo3l-verify/` (PR #286) — GitHub Action for capsule verification in CI
- `ci-plugins/{gitlab,circleci,jenkins}/` (PR #314) — verifier plugins for the 3 other major CI/CD platforms
- `crates/sbo3l-policy/src/mev_guard.rs` (PR #179) — pre-execution slippage + recipient allowlist
- `crates/sbo3l-execution/src/uniswap_trading.rs` (PR #165) — Uniswap V3 swap construction
- `crates/sbo3l-server/src/lib.rs` `/v1/admin/metrics` endpoint (PR #273)

### Docs + proof artifacts

- `docs/quickstart/` (PR #224) — 5 bounty × framework guides
- `docs/integrations/mcp-clients/` (PR #199) — Claude Desktop / Cursor / Continue config
- `docs/release/v1.2.0-recovery-runbook.md` (PR #228)
- `docs/proof/cross-protocol-killer-walkthrough.md` (PR #273)
- `docs/proof/cross-protocol-live-mock-2026-05-02.{json,txt,*-verify.txt}` (PR #319)
- `docs/proof/quickstart-passing.md`, `marketplace-adoption-flow.md`, `sdk-install-final.md`, `npm-publishes-final.md` (this PR)

### CI workflows

- `.github/workflows/integrations-publish.yml` (PR #145) — per-package release matrix
- `.github/workflows/sdk-install-matrix.yml` (PR #239) — registry liveness probe
- `.github/workflows/quickstart-validation.yml` (PR #260) — quickstart drift detection

## What's still pending

- **15 more npm publishes** — workflow matrix needs the post-cohort packages added. ~30 minutes; documented in `docs/proof/npm-publishes-final.md`.
- **2 more PyPI publishes** — need trusted publishers configured (`pypi-agno-py`, `pypi-pydantic-ai-py`). ~3 minutes each on PyPI.
- **Discord + Teams bot deployments** — Vercel deploy + Slack/Discord/Azure app registration; Daniel-side per `apps/chatops-{discord,teams}/DEPLOY.md`.
- **VS Code / JetBrains extension code** — deferred (P4 from R13/R14 — needs marketplace publishing accounts).
- **Browser extension code** — deferred (P5 from R13/R14 — needs Chrome Web Store + Firefox AMO accounts).
- **5 sponsor SDKs** — deferred (P2 from R14 — each is a deeper integration deserving its own focused turn).
- **Cross-protocol LIVE recording** — script ships in PR #319; recording is Daniel's one-shot per `RECORDING.md`.

See `docs/dev2/scope-cuts-r13-r14.md` for the explicit deferral spec.
