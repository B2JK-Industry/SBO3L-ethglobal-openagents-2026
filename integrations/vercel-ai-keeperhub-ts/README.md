# `@sbo3l/vercel-ai-keeperhub`

> Vercel AI SDK tool that **gates KeeperHub workflow execution through SBO3L's policy boundary**. Composable with `@sbo3l/sdk`. Edge-runtime compatible.

## Why this exists alongside `langchain-keeperhub` (Devendra's npm pkg)

| | `langchain-keeperhub` (Devendra) | `@sbo3l/vercel-ai-keeperhub` (this) |
|---|---|---|
| What it wraps | KH webhook execution | SBO3L policy gate → KH webhook execution |
| Decision step | x (agent decides) | yes (SBO3L decides; signed receipt) |
| Budget enforcement | x | yes |
| Audit chain | x | yes (hash-chained Ed25519 log) |
| Framework | LangChain (TS + Py) | Vercel AI SDK (TS, Edge runtime) |
| ENS / Turnkey TEE / MCP bridge | yes | x (not duplicated) |

**Composable:** use Devendra's tool for the raw KH binding + ours as the policy gate that decides whether the raw call should fire. Or use ours alone for the full gate-then-execute path.

## Why a Vercel-AI-flavored variant

The Vercel AI SDK is the fastest-growing TypeScript agent framework — the de-facto choice for Edge-runtime / Next.js Route Handlers. This package plugs in as an `ai.tool()` directly into `streamText` / `generateText`'s `tools` map: the LLM gets a typed `parameters` schema (zod) and the tool result is a plain JS object the LLM can branch on.

## Install

```bash
npm install @sbo3l/vercel-ai-keeperhub @sbo3l/sdk ai zod
```

## 5-line setup

```ts
import { streamText } from "ai";
import { openai } from "@ai-sdk/openai";
import { SBO3LClient } from "@sbo3l/sdk";
import { sbo3lVercelAIKeeperHubTool } from "@sbo3l/vercel-ai-keeperhub";

const client = new SBO3LClient({ endpoint: "http://localhost:8730" });
const result = streamText({
  model: openai("gpt-4o"),
  tools: { sbo3lKeeperHub: sbo3lVercelAIKeeperHubTool({ client }) },
  prompt: "Pay 0.05 USDC for an inference call via the KeeperHub workflow.",
});
```

## Wire path

1. Tool input (LLM-supplied): `{ aprp: { ...APRP body... } }` — typed object via the zod `parameters` schema (NOT a JSON-stringified string).
2. POST to SBO3L daemon `/v1/payment-requests`.
3. SBO3L decides allow / deny / requires_human against the loaded policy + budget + nonce + provider trust list.
4. On allow: SBO3L's `executor_callback` hands the signed `PolicyReceipt` to the daemon-side KeeperHub adapter (configured via `SBO3L_KEEPERHUB_WEBHOOK_URL` + `SBO3L_KEEPERHUB_TOKEN` env vars on the **daemon** process — not on the agent).
5. KH adapter POSTs the IP-1 envelope to the workflow webhook, captures `executionId`, surfaces it as `receipt.execution_ref`.
6. Tool returns (as an object, not stringified):
   ```json
   {
     "decision": "allow",
     "kh_workflow_id_advisory": "m4t4cnpmhv8qquce3bv3c",
     "kh_execution_ref": "kh-01HTAWX5...",
     "audit_event_id": "evt-...",
     "request_hash": "...", "policy_hash": "...",
     "matched_rule_id": "...", "deny_code": null
   }
   ```

## On `kh_workflow_id_advisory`

The `_advisory` suffix is intentional: today the daemon's env-configured webhook URL is the source of truth for actual routing. The per-call `workflowId` you pass to `sbo3lVercelAIKeeperHubTool({ workflowId })` is surfaced in the envelope for **context tagging** / audit logs, not as a routing override. See [KeeperHub/cli#52](https://github.com/KeeperHub/cli/issues/52) for the proposed contract that would make per-call routing safe.

## API

```ts
sbo3lVercelAIKeeperHubTool({
  client: SBO3LClientLike,
  workflowId?: string,             // default: DEFAULT_KH_WORKFLOW_ID
  description?: string,
  idempotencyKey?: (input) => string,
}): ai.Tool
```

Returns an `ai.tool()` descriptor with `{ description, parameters (zod), execute }` ready to drop into `streamText` / `generateText`.

`SBO3LClientLike` = anything with `submit(request, opts?) => Promise<SBO3LSubmitResult>`. `@sbo3l/sdk`'s `SBO3LClient` matches nominally; mocks/fakes can implement just this for tests.

## Edge-runtime notes

The package has no Node-only dependencies and ships ESM + CJS + `.d.ts`. Works in Vercel Edge Functions, Cloudflare Workers, and Bun — anywhere `fetch` is available globally.

## License

MIT
