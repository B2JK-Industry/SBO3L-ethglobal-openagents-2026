# `@sbo3l/langflow`

LangFlow tool component adapter for SBO3L.

```bash
npm i @sbo3l/langflow @sbo3l/sdk
```

## Wiring

```ts
import { SBO3LClient } from "@sbo3l/sdk";
import { sbo3lLangFlowComponent } from "@sbo3l/langflow";

const sbo3l = new SBO3LClient({ endpoint: "http://localhost:8730" });
const component = sbo3lLangFlowComponent({ client: sbo3l });

// Hand `component.descriptor` to LangFlow's component registry.
// `component.build` is the runtime callable LangFlow invokes per tool node fire.
```

## Behaviour

- **allow** → `{ ok: true, data: PolicyReceipt, audit_event_id }`
- **deny / requires_human** → `{ ok: false, error: "policy.deny" | "policy.requires_human", deny_code, audit_event_id }`
- **transport fail** → `{ ok: false, error: "transport.failed", deny_code: null, audit_event_id: null }`

The build function never throws — denies + transport errors all surface as `{ ok: false, ... }` envelopes so the upstream LLM node can branch.

## Tests

```bash
npm test         # 9 vitest passing
npm run typecheck
npm run build    # ESM + CJS + d.ts
```
