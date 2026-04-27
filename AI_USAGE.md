# AI Usage Disclosure

We used AI tools as coding assistants and reviewers throughout the ETHGlobal Open Agents 2026 build of **Mandate**.

## Tools used

- **Claude Code** (Anthropic) — primary implementation orchestrator: code generation, multi-file refactors, test scaffolding, schema validation logic, demo scripts.
- **OpenAI Codex** (via GitHub PR review integration) — independent code review on the open PR.

## AI-assisted areas

- Rust workspace layout and crate scaffolding.
- APRP/policy/decision-token/audit-event JSON schema validation code.
- Cargo dependency selection guided by [`docs/spec/19_knowledge_base.md`](docs/spec/19_knowledge_base.md).
- Test corpus runner and contract tests.
- CI workflow (`.github/workflows/ci.yml`).
- Demo runner shell scripts.
- README, this file, and `SUBMISSION_NOTES.md`.

## Human-led areas

- Product direction, brand and pitch (`Mandate` — "Don't give your agent a wallet. Give it a mandate.").
- Sponsor prize selection (KeeperHub, ENS, Uniswap stretch).
- Architecture decisions (3-layer separation: agent boundary / policy / signer; hash-chained audit; signed receipts).
- Interface contracts: locked wire formats in [`docs/spec/17_interface_contracts.md`](docs/spec/17_interface_contracts.md).
- Threat model and trust boundaries.
- Final code review and submission decisions.
- Demo script narrative and judging walkthrough.

## Pre-hackathon planning artifacts

The pre-hackathon planning repository (`agent-vault-os`) contains the strategy, threat model, architecture, schemas, OpenAPI draft, golden/adversarial test corpus and demo-agent harness contract. These artifacts pre-date the hackathon and were copied verbatim into [`docs/spec/`](docs/spec/) at the start of this build:

- `docs/spec/00_README.md` … `docs/spec/33_*.md` — full numbered planning doc set.
- `docs/spec/openapi.snapshot.json` — frozen OpenAPI 3.1 snapshot.

The live, evolving versions of `schemas/`, `test-corpus/` and `demo-agents/research-agent/` were also seeded from the planning repo and are now part of the implementation.

All implementation code (Rust crates, demo scripts, CI, sponsor adapters) was written during the hackathon.

## Honesty statement

- AI-generated code was reviewed and adapted by the human developer before commit.
- No AI-generated content is presented as if it were unaided human work.
- AI tooling never made architectural decisions without explicit human direction.
