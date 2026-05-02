# `@sbo3l/agentforce`

SBO3L adapter for the **Agentforce** agent / API surface.

```bash
npm i @sbo3l/agentforce @sbo3l/sdk
```

## Wiring

```ts
import { SBO3LClient } from "@sbo3l/sdk";
import { sbo3lAgentforceAction, runSbo3lAgentforceAction } from "@sbo3l/agentforce";

const sbo3l = new SBO3LClient({ endpoint: "http://localhost:8730" });
const tool = sbo3lAgentforceAction({ client: sbo3l });

// Hand `tool.descriptor` to Agentforce's tool registry.
// Pair each emitted tool call with `runSbo3lAgentforceAction`.
```

## Behaviour

- **allow** → `{ ok: true, output: PolicyReceipt }`
- **deny / requires_human** → `{ ok: false, output: { error, deny_code, audit_event_id } }`
- **transport fail** → `{ ok: false, output: { error: "transport.failed", detail } }`

The runner never re-throws — every outcome is a structured envelope so the framework's loop can branch and self-correct.

## Tests

```bash
npm test         # 11 vitest passing
npm run typecheck
npm run build    # ESM + CJS + d.ts
```
