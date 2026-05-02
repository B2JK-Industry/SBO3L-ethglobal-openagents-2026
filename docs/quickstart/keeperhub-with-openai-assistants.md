# KeeperHub × OpenAI Assistants — 5-min quickstart

Run an OpenAI Assistant that submits a payment intent through SBO3L and fires a real KeeperHub workflow on `allow`.

**Bounty:** KeeperHub
**Framework:** OpenAI Assistants API (TypeScript)
**Time:** 5 min

## 1. Install

```bash
mkdir kh-quickstart && cd kh-quickstart
npm init -y && npm pkg set type=module
npm i @sbo3l/sdk @sbo3l/openai-assistants openai tsx
export OPENAI_API_KEY=sk-...
```

## 2. Configure

SBO3L daemon at `http://localhost:8730` (see [common prerequisites](index.md#common-prerequisites-all-guides)). KH workflow id is wired daemon-side.

## 3. Code (`agent.ts`)

```ts
import OpenAI from "openai";
import { SBO3LClient } from "@sbo3l/sdk";
import { sbo3lAssistantTool, runSbo3lToolCall } from "@sbo3l/openai-assistants";

const openai = new OpenAI();
const sbo3l = new SBO3LClient({ endpoint: "http://localhost:8730" });
const tool = sbo3lAssistantTool({ client: sbo3l });

const aprp = {
  agent_id: "research-agent-01",
  task_id: "kh-quickstart-1",
  intent: "purchase_api_call" as const,
  amount: { value: "0.05", currency: "USD" as const },
  token: "USDC",
  destination: {
    type: "x402_endpoint" as const,
    url: "https://api.example.com/v1/inference",
    method: "POST" as const,
    expected_recipient: "0x1111111111111111111111111111111111111111",
  },
  payment_protocol: "x402" as const,
  chain: "base",
  provider_url: "https://api.example.com",
  expiry: new Date(Date.now() + 5 * 60 * 1000).toISOString(),
  nonce: globalThis.crypto.randomUUID(),
  risk_class: "low" as const,
};

const assistant = await openai.beta.assistants.create({
  model: "gpt-4o-mini",
  tools: [tool.definition],
  instructions: "Always call sbo3l_payment_request before paying.",
});
const thread = await openai.beta.threads.create({});
await openai.beta.threads.messages.create(thread.id, {
  role: "user",
  content: `Submit this APRP via the tool: ${JSON.stringify(aprp)}`,
});
let run = await openai.beta.threads.runs.create(thread.id, { assistant_id: assistant.id });

while (run.status !== "completed") {
  if (run.status === "requires_action" && run.required_action?.type === "submit_tool_outputs") {
    const outputs = await Promise.all(
      run.required_action.submit_tool_outputs.tool_calls.map((c) => runSbo3lToolCall(tool, c)),
    );
    run = await openai.beta.threads.runs.submitToolOutputs(thread.id, run.id, { tool_outputs: outputs });
    continue;
  }
  await new Promise((r) => setTimeout(r, 800));
  run = await openai.beta.threads.runs.retrieve(thread.id, run.id);
}

const msgs = await openai.beta.threads.messages.list(thread.id, { order: "desc", limit: 1 });
const reply = msgs.data[0]?.content[0];
if (reply?.type === "text") console.log(reply.text.value);
```

## 4. Run

```bash
npx tsx agent.ts
```

## 5. What you'll see

The Assistant calls the SBO3L tool, the runner returns a `PolicyReceipt` JSON, and the Assistant summarises:

```
The payment intent was approved. Audit event id: evt-01HTAWX5K3R8YV9NQB7C6P2DGR.
KeeperHub execution ref: kh-01HTAWX5K3R8YV9NQB7C6P2DGS.
```

## 6. Troubleshoot

- **`protocol.nonce_replay`** — `nonce` must be a fresh UUID per call. The snippet uses `crypto.randomUUID()`.
- **`policy.deny_recipient_not_allowlisted`** — use exactly `0x1111111111111111111111111111111111111111` for `chain: base`.
- **Run stuck on `requires_action`** — your loop must call `submitToolOutputs` for every tool_call. The runner above handles this.
- **Module-not-found on `@sbo3l/openai-assistants`** — make sure your `package.json` has `"type": "module"` (`npm pkg set type=module`).
- **Old `node`** — Node 18+ required for global `crypto.randomUUID`.

## Next

- [KH × LangChain (Python)](keeperhub-with-langchain.md)
- [Anthropic variant](ens-with-anthropic.md) (different bounty, similar flow)
