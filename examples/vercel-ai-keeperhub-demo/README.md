# vercel-ai-keeperhub-demo

> Minimum viable Vercel AI SDK demo of `@sbo3l/vercel-ai-keeperhub` — the SBO3L policy gate wired to a KeeperHub workflow.

## What it shows

- The `@sbo3l/vercel-ai-keeperhub` package as a Vercel AI SDK `tool()` plugged into `streamText`.
- The same tool callable directly via `tool.execute({ aprp }, {})` — so you can verify the wire path with **no OpenAI API key**.
- The full envelope returned by the tool: `{ decision, kh_workflow_id_advisory, kh_execution_ref, audit_event_id, request_hash, policy_hash, matched_rule_id, deny_code }`.

## Run locally (no OpenAI key needed)

```bash
# 1. Start sbo3l-server in mock mode (terminal A)
SBO3L_ALLOW_UNAUTHENTICATED=1 SBO3L_SIGNER_BACKEND=dev SBO3L_DEV_ONLY_SIGNER=1 \
  cargo run --bin sbo3l-server

# 2. Run the demo (terminal B)
cd examples/vercel-ai-keeperhub-demo
npm install
node agent.mjs
```

Expected output: an envelope with `decision: "allow"` and a populated `kh_execution_ref` (assuming the bundled reference policy + a configured KeeperHub adapter).

## Run as a Vercel Edge function

The same `agent.mjs` exports a `handler(req)` function ready to drop into a Next.js Route Handler:

```ts
// app/api/agent/route.ts
export { handler as POST } from "./agent.mjs";
```

Requires `OPENAI_API_KEY` at runtime. The LLM picks the `sbo3lKeeperHub` tool when the user asks for a KH-routed payment.

## Environment

- `SBO3L_ENDPOINT` (optional, default `http://localhost:8730`) — daemon address.
- `OPENAI_API_KEY` (Edge handler path only) — for the LLM calling the tool.
- Daemon-side: `SBO3L_KEEPERHUB_WEBHOOK_URL` + `SBO3L_KEEPERHUB_TOKEN` for live KH execution. Without those, the daemon returns `decision: allow` but `kh_execution_ref: null`.
