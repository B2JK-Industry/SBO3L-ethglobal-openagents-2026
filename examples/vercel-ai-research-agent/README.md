# `examples/vercel-ai-research-agent`

End-to-end Vercel AI SDK research agent with two tools (`data_fetch` + `sbo3l_payment_request`) that gates every payment through SBO3L's policy boundary. Routes allowed payments through KeeperHub workflow `m4t4cnpmhv8qquce3bv3c`.

Differs from `examples/vercel-ai-agent/` (the minimal one) in that this one **reasons across two tools** — fetch then decide-to-pay — using `generateText` with `maxSteps: 6` so the LLM can chain calls.

## 3-line setup

```bash
SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server &
cd examples/vercel-ai-research-agent && npm install
npm run smoke   # no OpenAI key needed
```

## With an LLM (full reasoning loop)

```bash
export OPENAI_API_KEY=sk-...
npm run agent "Pay 0.05 USDC for an inference call to api.example.com."
```

The agent (running on `gpt-4o-mini`) inspects the provider via `data_fetch`, then submits an APRP through `sbo3l_payment_request`. SBO3L decides allow/deny; on allow, the daemon's KH adapter routes to KeeperHub workflow `m4t4cnpmhv8qquce3bv3c`. On deny, the tool throws `PolicyDenyError`, the LLM sees the `denyCode`, and explains.

## Tools

| Tool | Description |
|---|---|
| `data_fetch` | GET a JSON URL, return body (zod-typed `{url}` parameter). |
| `sbo3l_payment_request` | APRP submit via `@sbo3l/vercel-ai`'s `sbo3lTool({ client })`. Returns the signed `PolicyReceipt` on allow; throws `PolicyDenyError` on deny. |

## Expected smoke output

```
▶ smoke: KH workflow target = m4t4cnpmhv8qquce3bv3c

▶ tool: data_fetch (GitHub status — public, low-noise)
  ✓ HTTP 200

▶ tool: sbo3l_pay (APRP → SBO3L → KH adapter)
✓ allow — execution_ref kh-01HTAWX5K3R8YV9NQB7C6P2DGS
  audit_event_id: evt-...
  signature.algorithm: ed25519
  signature.key_id: decision-mock-v1
```

Total wall-clock: < 30 s on a laptop with the daemon already running.

## Files

- `src/tools.ts` — `dataFetchTool` + `buildSbo3lTool` (real `@sbo3l/sdk` SBO3LClient).
- `src/agent.ts` — full `generateText` agent loop (needs `OPENAI_API_KEY`).
- `src/smoke.ts` — no-OpenAI-key smoke; calls each tool's `execute()` directly.

## License

MIT
