# `@sbo3l/letta`

SBO3L adapter for the **Letta** (formerly MemGPT) agent framework.

```bash
npm i @sbo3l/letta @sbo3l/sdk
```

## Wiring

```ts
import { SBO3LClient } from "@sbo3l/sdk";
import { sbo3lLettaTool, runSbo3lLettaToolCall } from "@sbo3l/letta";

const sbo3l = new SBO3LClient({ endpoint: "http://localhost:8730" });
const tool = sbo3lLettaTool({ client: sbo3l });

// Hand `tool.descriptor` to Letta's POST /tools registration.
// When Letta emits a tool_call, dispatch via the runner:
const result = await runSbo3lLettaToolCall(tool, toolCall);
// → result.ok / result.output (JSON-encoded receipt or deny envelope)
```

## Behaviour

- **allow** → `{ ok: true, output: PolicyReceipt }`
- **deny / requires_human** → `{ ok: false, output: { error, deny_code, audit_event_id } }`
- **transport fail** → `{ ok: false, output: { error: "transport.failed", detail } }`

The runner never re-throws — every outcome is a structured envelope so Letta's persistent-memory conversation loop can branch and self-correct.

## Tests

```bash
npm test         # 9 vitest passing
npm run typecheck
npm run build    # ESM + CJS + d.ts
```
