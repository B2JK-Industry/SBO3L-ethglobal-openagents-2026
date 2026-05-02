# `examples/elizaos-research-agent`

End-to-end ElizaOS research agent demo — exercises `@sbo3l/elizaos`'s plugin shape and `SBO3L_PAYMENT_REQUEST` Action against a real SBO3L daemon. Routes through KH workflow `m4t4cnpmhv8qquce3bv3c`.

> ElizaOS's runtime is heavyweight + preview API. This demo bypasses ElizaOS bootstrap and drives the plugin's Action directly via a synthetic Eliza-shaped message. The plugin code path is identical to what runs inside an ElizaOS character — `validate(runtime, message) → handler(runtime, message, state, options, callback)`.

## 3-line setup

```bash
SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server &
cd examples/elizaos-research-agent && npm install
npm run smoke   # no OpenAI key needed
```

## Inside a real ElizaOS character

```ts
import { sbo3lPlugin } from "@sbo3l/elizaos";
import { SBO3LClient } from "@sbo3l/sdk";

const client = new SBO3LClient({ endpoint: process.env.SBO3L_ENDPOINT! });

export const character = {
  name: "ResearchAgent",
  plugins: [sbo3lPlugin({ client })],
  // ... rest of your character config
};
```

The plugin registers one Action `SBO3L_PAYMENT_REQUEST` (similes: PAY, PURCHASE, SUBMIT_PAYMENT, REQUEST_PAYMENT). It triggers when a message contains `content.aprp` (object) or `content.text` (JSON-stringified APRP).

## Files

- `src/tools.ts` — `fetchUrl` + `buildSbo3lPlugin` (real `@sbo3l/sdk` SBO3LClient).
- `src/agent.ts` — synthetic Eliza-shaped runtime; full action lifecycle (validate + handler + callback).
- `src/smoke.ts` — minimal smoke; calls the action handler directly with a hardcoded APRP.

## License

MIT
