# `examples/mastra-research-agent`

Mastra agent that gates every payment-shaped action through SBO3L.

```
npm install
SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server &
npm run smoke                                  # no LLM, deterministic
```

## What this proves

- `@sbo3l/mastra` ships a Mastra `Tool` descriptor with zod input + output schemas.
- `tool.execute({ context })` matches the call shape Mastra would invoke at runtime.
- On allow: returns `{ decision, audit_event_id, execution_ref, receipt }`.
- On deny / requires_human: throws `PolicyDenyError` so Mastra surfaces a tool-execution error and the LLM can branch.

## Wiring into a Mastra Agent

```ts
import { Agent } from "@mastra/core/agent";
import { openai } from "@ai-sdk/openai";
import { SBO3LClient } from "@sbo3l/sdk";
import { sbo3lTool } from "@sbo3l/mastra";

const client = new SBO3LClient({ endpoint: "http://localhost:8730" });
const agent = new Agent({
  name: "research-agent",
  model: openai("gpt-4o"),
  tools: { sbo3l_payment_request: sbo3lTool({ client }) },
});

const r = await agent.generate(
  "Pay 0.05 USDC for an inference call against api.example.com.",
);
```

## Out of scope

- Workflow / step composition — focused on the single SBO3L tool. Compose with Mastra Workflows by wiring this descriptor into a step's `tools: { ... }` map.
- Memory / RAG integration — orthogonal to SBO3L's policy gate.
