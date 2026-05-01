# `@sbo3l/elizaos`

ElizaOS plugin wrapping SBO3L. Drop into your character's plugin list to gate every payment intent through SBO3L.

> ⚠ **DRAFT (T-1-5):** depends on F-9 (`@sbo3l/sdk`).

## Install

```bash
npm install @sbo3l/elizaos @sbo3l/sdk
```

## Usage

```ts
import { SBO3LClient } from "@sbo3l/sdk";
import { sbo3lPlugin } from "@sbo3l/elizaos";

const client = new SBO3LClient({ endpoint: "http://localhost:8730" });
const plugin = sbo3lPlugin({ client });

// Pass `plugin` into your character's `plugins: [...]`. The plugin
// registers an Action `SBO3L_PAYMENT_REQUEST` (similes: PAY, PURCHASE,
// SUBMIT_PAYMENT, REQUEST_PAYMENT) that fires when the agent's message
// contains an APRP (in `message.content.aprp` as a JSON object, or in
// `message.content.text` as a JSON-stringified APRP).
```

## What it does

The agent emits a payment intent as either a structured `aprp` object on
the message content or a JSON-stringified APRP in the text. The Action
forwards to `SBO3LClient.submit()` and replies with:

```json
{
  "decision": "allow",
  "execution_ref": "kh-...",
  "audit_event_id": "evt-...",
  ...
}
```

On `deny`, the envelope contains `deny_code` (e.g. `policy.budget_exceeded`)
so the agent can self-correct.

## Custom APRP extraction

```ts
sbo3lPlugin({
  client,
  extractAprp: (message) => myCharacter.parseIntoAprp(message),
});
```

## License

MIT
