# `@sbo3l/langchain`

LangChain JS Tool wrapping SBO3L. Drop into any LangChain agent's tool list to gate every payment intent through SBO3L's policy boundary.

> ⚠ **DRAFT (T-1-1):** depends on F-9 (`@sbo3l/sdk`) merging + publishing.

## Install

```bash
npm install @sbo3l/langchain @sbo3l/sdk
```

## Quick start

```ts
import { SBO3LClient } from "@sbo3l/sdk";
import { DynamicTool } from "@langchain/core/tools";
import { sbo3lTool } from "@sbo3l/langchain";

const client = new SBO3LClient({
  endpoint: "http://localhost:8730",
  auth: { kind: "bearer", token: process.env.SBO3L_BEARER_TOKEN! },
});

const tool = new DynamicTool(sbo3lTool({ client }));
// Pass `tool` into your LangChain agent's tool list.
```

## What it does

The agent emits a tool call with a JSON-stringified APRP. The Tool forwards
to `SBO3LClient.submit()` and returns a JSON envelope:

```json
{
  "decision": "allow",
  "deny_code": null,
  "matched_rule_id": "allow-low-risk-x402",
  "execution_ref": "kh-01HTAWX5K3R8YV9NQB7C6P2DGS",
  "audit_event_id": "evt-...",
  "request_hash": "...",
  "policy_hash": "..."
}
```

On `deny`, the LLM sees `deny_code` (`policy.budget_exceeded`,
`policy.token_unsupported`, etc.) and can self-correct or escalate.

## Idempotency

```ts
const tool = new DynamicTool(sbo3lTool({
  client,
  idempotencyKey: (aprp) => `${aprp.task_id}-${aprp.nonce}`,
}));
```

## Errors

Transport / auth failures surface as a JSON envelope with `error` (RFC 7807
domain code, e.g. `auth.required`) and `status`.

## Compatibility

- Node ≥ 18
- LangChain JS 0.1+ (compatible with `DynamicTool`, `DynamicStructuredTool`, or any tool factory)
- SBO3L daemon 0.1+

## License

MIT
