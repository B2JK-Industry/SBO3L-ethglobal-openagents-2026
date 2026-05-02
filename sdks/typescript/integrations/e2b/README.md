# `@sbo3l/e2b`

SBO3L adapter for the **E2B** agent / API surface.

```bash
npm i @sbo3l/e2b @sbo3l/sdk
```

## Wiring

```ts
import { SBO3LClient } from "@sbo3l/sdk";
import { sbo3lE2BGate, runSbo3lE2BGate } from "@sbo3l/e2b";

const sbo3l = new SBO3LClient({ endpoint: "http://localhost:8730" });
const tool = sbo3lE2BGate({ client: sbo3l });

// Hand `tool.descriptor` to E2B's tool registry.
// Pair each emitted tool call with `runSbo3lE2BGate`.
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
