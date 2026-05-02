# `@sbo3l/autogpt`

SBO3L adapter for the **AutoGPT** agent framework.

```bash
npm i @sbo3l/autogpt @sbo3l/sdk
```

## Wiring

```ts
import { SBO3LClient } from "@sbo3l/sdk";
import { sbo3lCommand, runSbo3lCommand } from "@sbo3l/autogpt";

const sbo3l = new SBO3LClient({ endpoint: "http://localhost:8730" });
const tool = sbo3lCommand({ client: sbo3l });

// Hand `tool.descriptor` to AutoGPT's tool registry.
// Pair each emitted tool call with `runSbo3lCommand` to dispatch through SBO3L.
```

## Behaviour

- **allow** → `{ ok: true, output: PolicyReceipt }`
- **deny / requires_human** → `{ ok: false, output: { error, deny_code, audit_event_id } }`
- **transport fail** → `{ ok: false, output: { error: "transport.failed", detail } }`

The runner never re-throws — every outcome is a structured envelope so the framework's loop can branch and self-correct.

## Tests

```bash
npm test         # 9 vitest passing
npm run typecheck
npm run build    # ESM + CJS + d.ts
```
