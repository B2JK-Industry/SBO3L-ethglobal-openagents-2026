# Mandate

> Don't give your agent a wallet. Give it a mandate.

**Mandate** is a local policy, budget, receipt and audit firewall that decides whether an autonomous AI agent may execute an onchain or payment action.

This repository was implemented during **ETHGlobal Open Agents 2026**. Planning and specification artifacts under [`docs/spec/`](docs/spec/) were copied from a pre-hackathon planning repository (`agent-vault-os`) and are clearly labelled as such — they are not prior product code.

---

## Status

Implementation complete. All hardening PRs (#5, #6, #7, #8, #9, #11) are merged into `main`; `cargo test --workspace --all-targets` runs **96/96 green**; `bash demo-scripts/run-openagents-final.sh` runs end-to-end clean. See [`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md) for the post-merge snapshot and [`FINAL_REVIEW.md`](FINAL_REVIEW.md) for the submission-readiness audit.

## What this is

- A Rust workspace implementing the **Mandate** spending-mandate firewall for AI agents.
- A real research-agent demo harness that proves legitimate vs prompt-injection scenarios.
- Sponsor-facing adapters for **KeeperHub**, **ENS** and (stretch) **Uniswap**.
- Signed policy receipts and a tamper-evident audit hash chain.

## How to run the demo

From a fresh clone:

```bash
git clone https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026.git
cd mandate-ethglobal-openagents-2026
bash demo-scripts/run-openagents-final.sh
```

You need a Rust toolchain (workspace MSRV `1.80`) and Python 3 for the schema validators. The demo runs in ~5 seconds and exercises 12 verification gates (schema, locked golden hash, audit chain, policy/budget/storage/server tests, agent harness, ENS, KeeperHub, Uniswap, red-team prompt-injection, audit-chain tamper detection, and an explicit agent no-key boundary proof) plus a deterministic transcript artifact written to [`demo-scripts/artifacts/latest-demo-summary.json`](demo-scripts/artifacts/).

See [`SUBMISSION_NOTES.md`](SUBMISSION_NOTES.md) for the ETHGlobal submission narrative, [`SUBMISSION_FORM_DRAFT.md`](SUBMISSION_FORM_DRAFT.md) for copy-paste-ready ETHGlobal form text, and [`AI_USAGE.md`](AI_USAGE.md) for AI tooling disclosure.

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
