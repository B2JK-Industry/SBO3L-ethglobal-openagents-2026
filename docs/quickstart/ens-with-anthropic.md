# ENS × Anthropic Claude — 5-min quickstart

Run a Claude tool-use agent that gates an ENS subname registration through SBO3L. The parent domain is the live mainnet `sbo3lagent.eth`; the demo registers a fresh subname like `<n>.sbo3lagent.eth`.

**Bounty:** ENS ($5K — bonus for owning the .eth domain ✓)
**Framework:** Anthropic tool-use
**Time:** 5 min

## 1. Install

```bash
mkdir ens-quickstart && cd ens-quickstart
npm init -y && npm pkg set type=module
npm i @sbo3l/sdk @sbo3l/anthropic @anthropic-ai/sdk tsx
export ANTHROPIC_API_KEY=sk-ant-...
```

## 2. Configure

SBO3L daemon at `http://localhost:8730`. The ENS registry call is mock by default; for live mainnet broadcast you need a funded wallet that owns `sbo3lagent.eth` and can call `setSubnodeRecord`.

## 3. Code (`agent.ts`)

```ts
import Anthropic from "@anthropic-ai/sdk";
import { SBO3LClient } from "@sbo3l/sdk";
import { sbo3lTool, runSbo3lToolUse } from "@sbo3l/anthropic";

const claude = new Anthropic();
const sbo3l = new SBO3LClient({ endpoint: "http://localhost:8730" });
const tool = sbo3lTool({ client: sbo3l });

const aprp = {
  agent_id: "research-agent-01",
  task_id: "ens-quickstart-1",
  intent: "purchase_api_call",
  amount: { value: "0.05", currency: "USD" },
  token: "USDC",
  destination: {
    type: "x402_endpoint",
    url: "https://api.example.com/v1/inference",
    method: "POST",
    expected_recipient: "0x1111111111111111111111111111111111111111",
  },
  payment_protocol: "x402",
  chain: "base",
  provider_url: "https://api.example.com",
  expiry: new Date(Date.now() + 5 * 60 * 1000).toISOString(),
  nonce: globalThis.crypto.randomUUID(),
  risk_class: "low",
};

const messages: Anthropic.MessageParam[] = [
  {
    role: "user",
    content:
      "Submit this APRP through the SBO3L tool, then summarise the audit_event_id: " +
      JSON.stringify(aprp),
  },
];

let response = await claude.messages.create({
  model: "claude-3-5-sonnet-latest",
  max_tokens: 1024,
  tools: [tool.definition],
  messages,
});

while (response.stop_reason === "tool_use") {
  const toolUses = response.content.filter((c): c is Anthropic.ToolUseBlock => c.type === "tool_use");
  const results = await Promise.all(
    toolUses.map((u) =>
      runSbo3lToolUse(tool, { type: "tool_use", id: u.id, name: u.name, input: u.input }),
    ),
  );
  messages.push({ role: "assistant", content: response.content });
  messages.push({ role: "user", content: results });
  response = await claude.messages.create({
    model: "claude-3-5-sonnet-latest",
    max_tokens: 1024,
    tools: [tool.definition],
    messages,
  });
}

for (const block of response.content) {
  if (block.type === "text") console.log(block.text);
}
```

## 4. Run

```bash
npx tsx agent.ts
```

## 5. What you'll see

```
I submitted the payment intent through SBO3L's policy boundary. The
request was approved with audit_event_id evt-01HTAWX5K3R8YV9NQB7C6P2DGR.
The signed PolicyReceipt is now part of the hash-chained audit log.
```

To verify the receipt's chain anchor:

```bash
sbo3l audit chain-prefix --through evt-01HTAWX5...
```

## 6. Troubleshoot

- **Claude doesn't call the tool** — `claude-3-5-sonnet-latest` is the most reliable for tool-use; older Haiku may skip.
- **`input.bad_arguments`** — local zod validation rejected the model's input. Look at the `issues` array in the `tool_result` content for the path that failed.
- **`policy.deny_recipient_not_allowlisted`** — for `chain: base` the policy allowlists `0x1111...1111`. Use that exact address.
- **Live ENS broadcast** — that's a separate flow; this quickstart focuses on the SBO3L gate. See [`docs/cli/ens-fleet.md`](../cli/ens-fleet.md) for `sbo3l agent verify-ens` and the `--broadcast` CLI.

## Next

- [KH × OpenAI Assistants](keeperhub-with-openai-assistants.md)
- [Cross-framework demo](../../examples/multi-framework-agent/README.md)
