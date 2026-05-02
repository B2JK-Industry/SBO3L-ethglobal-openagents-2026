# SBO3L quickstart guides — bounty × framework matrix

Each guide is a **5-minute setup** that takes you from `npm install` (or `pip install`) to a single signed `PolicyReceipt` on a real bounty target.

## Guide matrix

| Bounty | TypeScript | Python |
|---|---|---|
| **KeeperHub** | [KH × OpenAI Assistants](keeperhub-with-openai-assistants.md) | [KH × LangChain](keeperhub-with-langchain.md) |
| **Uniswap** | [Uniswap × Vercel AI](uniswap-with-vercel-ai.md) · [Uniswap × Mastra](uniswap-with-mastra.md) | — |
| **ENS** | [ENS × Anthropic](ens-with-anthropic.md) | — |

## Common prerequisites (all guides)

```bash
# 1. Build + run the SBO3L daemon (one-time)
git clone https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026.git
cd SBO3L-ethglobal-openagents-2026
SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server &
# → daemon listens on http://localhost:8730

# 2. (only for live-mode bounties) Copy your Sepolia RPC + funded wallet env
export SBO3L_ETH_RPC_URL=https://eth-sepolia.g.alchemy.com/v2/<your-key>
export SBO3L_ETH_PRIVATE_KEY=0x...
```

Each guide assumes you've done these once. Skip ahead.

## Acceptance criteria (every guide)

- `npm install` (or `pip install`) finishes in <30s
- Smoke run completes in <5s
- Console prints a real `PolicyReceipt` with non-empty `audit_event_id`
- Re-running prints a *different* `audit_event_id` (replay protection works)

## Pattern across guides

All guides share the same shape:

1. Install (3 lines)
2. Configure (3 lines)
3. Code snippet (~30 lines, copy-paste runnable)
4. Run (1 line)
5. What you'll see (sample output)
6. Troubleshoot (3-5 common issues)

If a guide takes more than 5 minutes, it's a bug — file an issue.
