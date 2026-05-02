# `@sbo3l/langchain-keeperhub`

> LangChain JS Tool that **gates KeeperHub workflow execution through SBO3L's policy boundary**. Composable with `@sbo3l/sdk`.

## Why this exists alongside `langchain-keeperhub` (Devendra's npm pkg)

| | `langchain-keeperhub` (Devendra) | `@sbo3l/langchain-keeperhub` (this) |
|---|---|---|
| What it wraps | KH webhook execution | SBO3L policy gate → KH webhook execution |
| Decision step | ✗ (agent decides) | ✓ (SBO3L decides; signed receipt) |
| Budget enforcement | ✗ | ✓ |
| Audit chain | ✗ | ✓ (hash-chained Ed25519 log) |
| ENS / Turnkey TEE / MCP bridge | ✓ | ✗ (not duplicated) |

**Composable:** use Devendra's tool for the raw KH binding + ours as the policy gate that decides whether the raw call should fire. Or use ours alone for the full gate-then-execute path.

## Install

```bash
npm install @sbo3l/langchain-keeperhub @sbo3l/sdk
```

## 5-line setup

```ts
import { SBO3LClient } from "@sbo3l/sdk";
import { sbo3lKeeperHubTool } from "@sbo3l/langchain-keeperhub";

const client = new SBO3LClient({ endpoint: "http://localhost:8730" });
const tool = sbo3lKeeperHubTool({ client });
// pass `tool` (or wrap as DynamicTool) into your LangChain agent's tool list
```

## Wire path

1. Tool input: JSON-stringified APRP.
2. POST to SBO3L daemon `/v1/payment-requests`.
3. SBO3L decides allow / deny / requires_human against the loaded policy + budget + nonce + provider trust list.
4. On allow: SBO3L's `executor_callback` hands the signed `PolicyReceipt` to the daemon-side KeeperHub adapter (configured via `SBO3L_KEEPERHUB_WEBHOOK_URL` + `SBO3L_KEEPERHUB_TOKEN` env vars on the **daemon** process — not on the agent).
5. KH adapter POSTs the IP-1 envelope to the workflow webhook, captures `executionId`, surfaces it as `receipt.execution_ref`.
6. Tool returns:
   ```json
   {
     "decision": "allow",
     "kh_workflow_id_advisory": "m4t4cnpmhv8qquce3bv3c",
     "kh_execution_ref": "kh-01HTAWX5...",
     "audit_event_id": "evt-...",
     "request_hash": "...", "policy_hash": "...",
     "matched_rule_id": "...", "deny_code": null
   }
   ```

## On `kh_workflow_id_advisory`

The `_advisory` suffix is intentional: today the daemon's env-configured webhook URL is the source of truth for actual routing. The per-call `workflowId` you pass to `sbo3lKeeperHubTool({ workflowId })` is surfaced in the envelope for **context tagging** / audit logs, not as a routing override. See [KeeperHub/cli#52](https://github.com/KeeperHub/cli/issues/52) for the proposed contract that would make per-call routing safe.

## API

```ts
sbo3lKeeperHubTool({
  client: SBO3LClientLike,
  workflowId?: string,             // default: DEFAULT_KH_WORKFLOW_ID
  name?: string,                   // default: "sbo3l_keeperhub_payment_request"
  description?: string,
  idempotencyKey?: (input) => string,
}): SBO3LKeeperHubToolDescriptor
```

`SBO3LClientLike` = anything with `submit(request, opts?) → Promise<SBO3LSubmitResult>`. `@sbo3l/sdk`'s `SBO3LClient` matches nominally; mocks/fakes can implement just this for tests.

## License

MIT
