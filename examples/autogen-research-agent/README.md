# `examples/autogen-research-agent`

End-to-end Microsoft AutoGen research agent demo using `@sbo3l/autogen`'s function descriptor shape. Two functions (`data_fetch` + `sbo3l_payment_request`), routed through SBO3L → KeeperHub workflow `m4t4cnpmhv8qquce3bv3c`.

The function descriptors built by `@sbo3l/autogen` plug into any AutoGen runtime that accepts `{name, description, parameters, async call}` (e.g. `ConversableAgent.register_for_llm`). This demo drives them via the OpenAI function-calling API directly — same shape AutoGen forwards to the LLM.

## 3-line setup

```bash
SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server &
cd examples/autogen-research-agent && npm install
npm run smoke   # no OpenAI key needed
```

## With an LLM

```bash
export OPENAI_API_KEY=sk-...
npm run agent "Pay 0.05 USDC for an inference call to api.example.com."
```

The agent loops up to 6 steps: LLM picks a function, demo invokes the descriptor's `call()`, result is fed back, LLM continues. Tool calls printed inline so you can see the reasoning trace.

## Functions

| Function | Description |
|---|---|
| `data_fetch` | GET a JSON URL, return body (zod-equivalent JSON Schema constrains LLM args). |
| `sbo3l_payment_request` | APRP submit via `@sbo3l/autogen`'s `sbo3lFunction({ client })`. Returns flat decision envelope; on `deny`, `decision === "deny"` and `deny_code` set; on transport failure, `error` set. |

## Expected smoke output

```
▶ smoke: KH workflow target = m4t4cnpmhv8qquce3bv3c

▶ function: data_fetch (GitHub status — public, low-noise)
  ✓ HTTP 200

▶ function: sbo3l_payment_request (APRP → SBO3L → KH adapter)
  envelope:
    decision: "allow"
    execution_ref: "kh-..."
    audit_event_id: "evt-..."
    ...

✓ allow — execution_ref kh-...
  audit_event_id: evt-...
```

## License

MIT
