# `examples/langchain-ts-research-agent`

End-to-end LangChain JS research agent that asks SBO3L to authorize every payment-shaped action. Reasons across 2 tools (`data_fetch` for research, `sbo3l_payment_request` for paying) and routes allowed payments through the live KeeperHub workflow `m4t4cnpmhv8qquce3bv3c` (verified end-to-end on 2026-04-30).

## 3-line setup

```bash
SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server &
cd examples/langchain-ts-research-agent && npm install
npm run smoke   # no OpenAI key needed
```

## With an LLM (full reasoning loop)

```bash
export OPENAI_API_KEY=sk-...
npm run agent "Pay 0.05 USDC for an inference call to api.example.com."
```

The OpenAI-functions agent picks the tool calls itself, fetches provider metadata via `data_fetch`, then submits an APRP through `sbo3l_payment_request`. SBO3L decides allow/deny; on allow, the daemon's KH adapter routes to KeeperHub workflow `m4t4cnpmhv8qquce3bv3c`.

## Tools

| Tool | Description |
|---|---|
| `data_fetch` | GET a JSON URL, return body. The agent uses it to inspect a provider before paying. |
| `sbo3l_payment_request` | Submit an APRP via `@sbo3l/langchain` → SBO3L policy boundary → KH adapter. |

## Expected smoke output

```
▶ smoke: KH workflow target = m4t4cnpmhv8qquce3bv3c

▶ tool: data_fetch (GitHub status — public, low-noise)
  ✓ HTTP 200

▶ tool: sbo3l_payment_request (APRP → SBO3L → KH adapter)
  envelope:
    decision: "allow"
    deny_code: null
    matched_rule_id: "allow-low-risk-x402"
    execution_ref: "kh-01HTAWX5K3R8YV9NQB7C6P2DGS"
    audit_event_id: "evt-..."
    request_hash: "..."
    policy_hash: "..."

✓ allow — execution_ref kh-01HTAWX5K3R8YV9NQB7C6P2DGS
  audit_event_id: evt-...
```

Total wall-clock: < 30 s on a laptop with the daemon already running.

## Files

- `src/tools.ts` — `data_fetch` + `sbo3l_payment_request` (via `@sbo3l/langchain`).
- `src/agent.ts` — full LangChain OpenAI-functions agent (needs `OPENAI_API_KEY`).
- `src/smoke.ts` — no-OpenAI-key smoke; exercises the tool path directly.

## On `deny`

The agent sees the deny envelope and can self-correct. Try:

```bash
npm run agent "Pay 1000 USDC for a high-risk dataset."
# → SBO3L denies (policy.budget_exceeded), agent explains and asks the user
```

## License

MIT
