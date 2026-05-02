# `@sbo3l/inngest`

Durable-workflow adapter for SBO3L. Gates each payment-shaped step through the policy boundary, with replay-safe audit anchoring.

```bash
npm i @sbo3l/inngest @sbo3l/sdk inngest
```

## Wiring

```ts
import { Inngest, NonRetriableError } from "inngest";
import { SBO3LClient } from "@sbo3l/sdk";
import { gateAprp, PolicyDenyError } from "@sbo3l/inngest";

const inngest = new Inngest({ id: "agent-runner" });
const sbo3l = new SBO3LClient({ endpoint: "http://sbo3l:8730" });

export const swap = inngest.createFunction(
  { id: "agent.swap" },
  { event: "agent/swap.requested" },
  async ({ event, step }) => {
    try {
      const receipt = await gateAprp(step, sbo3l, event.data.aprp);
      await step.run("execute-swap", () => doSwap(event.data, receipt));
    } catch (e) {
      if (e instanceof PolicyDenyError) {
        // Inngest treats throws as retries; wrap to skip retries on deterministic deny.
        throw new NonRetriableError(e.message);
      }
      throw e;
    }
  },
);
```

## Why a wrapper instead of raw `step.run`

Inngest persists each `step.run` result. On a workflow retry, the persisted result replays — handler doesn't re-execute. `gateAprp` makes this work for SBO3L by:

1. Wrapping submit in `step.run("sbo3l.submit:<task_id>", ...)` so the receipt is journaled.
2. Returning a non-throwing union from the inner handler so the deny envelope is **also** journaled (otherwise an exception would re-throw on every retry, re-fetching the daemon and tripping `protocol.nonce_replay`).
3. Re-throwing as `PolicyDenyError` AFTER replay so the caller can wrap with `NonRetriableError` — denies are deterministic; retrying is wasteful.

## `gateAprpSafe` — no-throw variant

```ts
const r = await gateAprpSafe(step, sbo3l, aprp);
if (r.ok) await doExecution(r.receipt);
else logDeny(r.decision, r.deny_code, r.audit_event_id);
```

Use when the workflow has its own deny-handling branch and shouldn't fall through to Inngest's retry / NonRetriable handling.

## Tests

```bash
npm test         # 10 vitest passing
npm run typecheck
npm run build
```
