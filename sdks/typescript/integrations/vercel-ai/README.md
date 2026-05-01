# `@sbo3l/vercel-ai`

Vercel AI SDK adapter for SBO3L. Drop into any `streamText` / `generateText` `tools` map to gate every payment intent through SBO3L's policy boundary.

## Install

```bash
npm install @sbo3l/vercel-ai @sbo3l/sdk ai zod
```

## 5-line wire-up

```ts
import { streamText } from "ai";
import { openai } from "@ai-sdk/openai";
import { SBO3LClient } from "@sbo3l/sdk";
import { sbo3lTool } from "@sbo3l/vercel-ai";

const client = new SBO3LClient({ endpoint: "http://localhost:8730" });

const result = streamText({
  model: openai("gpt-4o"),
  tools: { pay: sbo3lTool({ client }) },
  prompt: "Pay 0.05 USDC for an inference call.",
});
```

That's it. The LLM emits a `pay` tool call with an APRP body; SBO3L decides; the tool returns the signed `PolicyReceipt` (allow) or throws `PolicyDenyError` (deny).

## What you get back

**On `decision === "allow"`** — the tool returns the full v1 `PolicyReceipt`:

```json
{
  "receipt_type": "sbo3l.policy_receipt.v1",
  "decision": "allow",
  "execution_ref": "kh-01HTAWX5K3R8YV9NQB7C6P2DGS",
  "audit_event_id": "evt-...",
  "request_hash": "c0bd2fab...",
  "policy_hash": "e044f13c...",
  "signature": { "algorithm": "ed25519", "signature_hex": "..." }
}
```

**On `decision === "deny"`** — throws `PolicyDenyError`:

```ts
import { PolicyDenyError } from "@sbo3l/vercel-ai";

try {
  await result.consumeStream();
} catch (e) {
  if (e instanceof PolicyDenyError) {
    console.log("denied:", e.denyCode);   // e.g. "policy.budget_exceeded"
    console.log("audit event:", e.auditEventId);
  }
}
```

The Vercel AI SDK forwards tool-execute errors back to the LLM as the tool result, so the model can self-correct (lower the amount, switch tokens, etc.) or escalate.

## APRP zod schema

The tool's `parameters` is the `aprpSchema` zod object — exported for callers that want to compose it (e.g. wrapping in a `StructuredOutput`):

```ts
import { aprpSchema } from "@sbo3l/vercel-ai";
type AprpInput = z.infer<typeof aprpSchema>;
```

## Idempotency

```ts
sbo3lTool({
  client,
  idempotencyKey: (aprp) => `${aprp.task_id}-${aprp.nonce}`,
})
```

Same key + same body → cached envelope; same key + different body → HTTP 409.

## Compatibility

- `ai` ≥ 3.0 (tested against 3.4)
- `@sbo3l/sdk` ≥ 0.1.0
- Node ≥ 18
- Works in Next.js Route Handlers, Edge runtime, and standalone scripts.

## License

MIT
