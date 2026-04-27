# Submission Notes — ETHGlobal Open Agents 2026

## Project

- **Public brand:** Mandate
- **Tagline:** Spending mandates for autonomous agents.
- **Pitch line:** Don't give your agent a wallet. Give it a mandate.
- **Technical namespace:** `mandate` (crates, daemon, CLI, schema host).
- **Event:** ETHGlobal Open Agents 2026.

## What was built during the hackathon

All implementation code in this repository:

- Rust workspace (`crates/mandate-*`).
- `mandate` CLI (`mandate aprp validate`, `mandate verify-audit`, demo helpers).
- Strict APRP schema validation, decision token signing, policy receipts.
- Policy engine (Rego via `regorus`) + multi-scope budget checks.
- SQLite-backed storage with hash-chained audit log.
- Real research-agent harness with `legit-x402` and `prompt-injection` scenarios.
- ENS identity proof adapter (resolves agent → vault endpoint + policy hash + audit root).
- KeeperHub guarded-execution adapter (`mandate-execution::keeperhub`).
- Uniswap guarded-swap adapter (`mandate-execution::uniswap`): swap-policy guard (token allowlist, max notional, max slippage, quote freshness, treasury recipient) + `UniswapExecutor::local_mock()`.
- Sponsor demo scripts: `ens-agent-identity.sh`, `keeperhub-guarded-execution.sh`, `uniswap-guarded-swap.sh`.
- Standalone red-team gate: `demo-scripts/red-team/prompt-injection.sh`.
- `demo-scripts/run-openagents-final.sh` — single-command demo runner with audit-chain tamper detection.
- `demo-scripts/demo-video-script.md` — 3:30 storyboard with recording checklist.
- CI: fmt, clippy, tests (62 passing), schema validation.

## What was reused as planning material

The pre-hackathon planning repository (`agent-vault-os`) is included as planning material in [`docs/spec/`](docs/spec/) and clearly labelled. It contains:

- Strategic vision, threat model, architecture and policy model docs.
- JSON Schemas, OpenAPI draft, golden/adversarial test corpus, demo-agent harness contract.

These are documentation/specifications, not prior product code. See [`AI_USAGE.md`](AI_USAGE.md).

## Targeted partner prizes (max 3)

1. **KeeperHub — Best Use of KeeperHub.** Mandate is the pre-execution policy and risk layer; KeeperHub is the execution layer. Approved actions are routed to KeeperHub; denied actions never reach it.
2. **ENS — Best Integration for AI Agents.** ENS records resolve `mandate:agent_id`, `mandate:endpoint`, `mandate:policy_hash`, `mandate:audit_root`. ENS gates discovery and verifies the active policy hash matches.
3. **Uniswap — Best Uniswap API Integration (stretch).** Mandate enforces token allowlists, slippage caps, quote freshness and treasury policy before any agent-initiated swap is signed.

## What is live vs mocked

- APRP / policy / receipts / audit chain — **live**, end-to-end deterministic.
- ENS resolution — live against testnet records OR clearly-labelled local resolver fixture (see `demo-scripts/sponsors/ens-agent-identity.sh`).
- KeeperHub adapter — live against KeeperHub MCP/API; falls back to faithful local mock when credentials are unavailable, clearly disclosed in demo output.
- Uniswap adapter — live quote against Uniswap API where available; otherwise faithful local mock.

## Demo

```bash
bash demo-scripts/run-openagents-final.sh
```

Proves, in order:

1. Real agent identity (ENS-resolved).
2. Legitimate payment request → allowed → policy receipt → routed to KeeperHub.
3. Prompt-injection malicious request → denied **before execution** → deny code visible.
4. Audit chain verified end-to-end.
5. No private key ever held by the agent.

## Demo video

Target length 3:30, hard stop 3:50. Real human voice narration. Storyboard in [`docs/spec/30_ethglobal_submission_compliance.md`](docs/spec/30_ethglobal_submission_compliance.md) §7.

## Judging criteria mapping

| Criterion | Mandate angle |
|---|---|
| Technicality | Policy engine + signed receipts + audit chain + sponsor adapter + agent harness. |
| Originality | Spending mandate replaces agent wallet — agent never holds key. |
| Practicality | Local daemon + CLI + runnable demo; useful for agent builders today. |
| Usability | One-command final demo, readable receipts, clear deny codes. |
| WOW factor | Prompt-injection visibly tries to spend, Mandate denies pre-execution and proves why. |
