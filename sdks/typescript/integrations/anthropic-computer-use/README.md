# `@sbo3l/anthropic-computer-use`

SBO3L policy gate for Anthropic Claude's **built-in** computer-use / bash / text_editor tools.

```bash
npm i @sbo3l/anthropic-computer-use @sbo3l/sdk @anthropic-ai/sdk
```

## What this is for

`@sbo3l/anthropic` (the sibling package) wraps SBO3L as a *custom* function tool you register alongside Claude. **This** package gates the *built-in* tools Claude calls when given desktop access:

- `computer` — mouse / keyboard / screenshot
- `bash` — shell commands
- `text_editor` (`str_replace_editor`) — file edits

Without a gate, Claude can `bash`-execute or `click` arbitrary things. With the gate, every action flows through SBO3L's policy boundary first; on deny, the executor is skipped and Claude sees a structured `tool_result` it can branch on.

## Usage

```ts
import Anthropic from "@anthropic-ai/sdk";
import { SBO3LClient } from "@sbo3l/sdk";
import { gateComputerAction } from "@sbo3l/anthropic-computer-use";

const claude = new Anthropic();
const sbo3l = new SBO3LClient({ endpoint: "http://localhost:8730" });

// Your existing computer-use executor — runs xdotool / bash / fs ops
async function realExecutor(block) {
  if (block.name.startsWith("computer")) return runDesktopAction(block.input);
  if (block.name.startsWith("bash"))     return runBashCommand(block.input);
  return runTextEditor(block.input);
}

const response = await claude.messages.create({
  model: "claude-3-5-sonnet-latest",
  max_tokens: 1024,
  tools: [{ type: "computer_20241022", name: "computer", display_width_px: 1920, display_height_px: 1080, display_number: 1 }],
  messages: [{ role: "user", content: "Open the payment portal and click confirm." }],
});

const results = [];
for (const block of response.content) {
  if (block.type !== "tool_use") continue;
  const result = await gateComputerAction({
    sbo3l,
    block,
    agentId: "research-agent-01",
    executor: realExecutor,
  });
  results.push(result);
}
// ...push results back as the next user message
```

## Action classification + default risk

| Tool name | Action | Class | Default risk |
|---|---|---|---|
| `computer*` | `left_click`/`right_click`/`drag` | `computer.mouse` | `high` |
| `computer*` | `type`/`key` | `computer.keyboard` | `high` |
| `computer*` | `screenshot` | `computer.screenshot` | `low` |
| `bash*` | any | `bash.exec` | `high` |
| `str_replace_editor`/`text_editor*` | `view` | `text_editor.read` | `low` |
| `str_replace_editor`/`text_editor*` | `create`/`str_replace`/`insert` | `text_editor.write` | `medium` |
| anything else | — | `unknown` | `critical` (fail-closed) |

Override per-call via `riskClassifier`.

## APRP shape

Every gated action lands at SBO3L as:

```json
{
  "agent_id": "<your agent>",
  "task_id": "cu-<tool_use.id>",
  "intent": "pay_compute_job",
  "amount": { "value": "0", "currency": "USD" },
  "token": "USDC",
  "destination": { "type": "smart_account", "address": "0x000...0" },
  "payment_protocol": "smart_account_session",
  "chain": "base",
  "provider_url": "urn:anthropic-computer-use:<class>",
  "expiry": "<now + 5 min>",
  "nonce": "<UUID>",
  "risk_class": "<derived>"
}
```

Write policy rules that match on `provider_url` to gate per action class. Example: `input.provider_url == "urn:anthropic-computer-use:bash.exec" and input.risk_class == "high"` to deny all bash by default.

## Result envelopes

- **allow** → `{ ok: true, output: <executor result>, audit_event_id, action_class }`
- **deny / requires_human** → `is_error: true, content: { error: "policy.deny", deny_code, audit_event_id, action_class }`
- **executor throws** → `is_error: true, content: { error: "executor.failed", detail, audit_event_id, action_class }`
- **transport fails** → `is_error: true, content: { error: "transport.failed", detail, action_class }`

The `audit_event_id` is preserved on every path (where one was issued) so post-run audits correlate.

## Tests

```bash
npm test         # 21 vitest passing
npm run typecheck
npm run build    # ESM + CJS + d.ts
```
