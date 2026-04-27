# Mandate

> Don't give your agent a wallet. Give it a mandate.

**Mandate** is a local policy, budget, receipt and audit firewall that decides whether an autonomous AI agent may execute an onchain or payment action.

This repository was implemented during **ETHGlobal Open Agents 2026**. Planning and specification artifacts under [`docs/spec/`](docs/spec/) were copied from a pre-hackathon planning repository (`agent-vault-os`) and are clearly labelled as such — they are not prior product code.

---

## Status

Pre-implementation. Repo bootstrap in progress. See [`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md) for live progress.

## What this is

- A Rust workspace implementing the **Mandate** spending-mandate firewall for AI agents.
- A real research-agent demo harness that proves legitimate vs prompt-injection scenarios.
- Sponsor-facing adapters for **KeeperHub**, **ENS** and (stretch) **Uniswap**.
- Signed policy receipts and a tamper-evident audit hash chain.

## How to run the demo (when ready)

```bash
bash demo-scripts/run-openagents-final.sh
```

See [`SUBMISSION_NOTES.md`](SUBMISSION_NOTES.md) for the ETHGlobal submission narrative and [`AI_USAGE.md`](AI_USAGE.md) for AI tooling disclosure.

## Repository layout

```
crates/         Rust workspace crates (implementation)
demo-agents/    Real agent harness (research-agent)
demo-scripts/   Demo runners (final + per-sponsor)
schemas/        JSON Schema 2020-12 contracts (live)
test-corpus/    Golden + adversarial fixtures
migrations/     SQLite schema migrations
docs/api/       OpenAPI 3.1 contract
docs/spec/      Pre-hackathon planning artifacts (reference only)
.github/        CI workflows
```

## License

[MIT](LICENSE)
