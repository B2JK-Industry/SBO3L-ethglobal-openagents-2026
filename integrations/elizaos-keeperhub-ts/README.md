# `@sbo3l/elizaos-keeperhub`

> ElizaOS Action that **gates KeeperHub workflow execution through SBO3L's policy boundary**. Composable with `@sbo3l/sdk` and `@sbo3l/elizaos`.

## Why this exists alongside Bleyle's ElizaOS plugin

| | Bleyle's ElizaOS plugin | `@sbo3l/elizaos-keeperhub` (this) |
|---|---|---|
| What it wraps | KH webhook execution | SBO3L policy gate → KH webhook execution |
| Decision step | execution-only (agent decides) | SBO3L decides; signed receipt |
| Budget enforcement | no | yes |
| Audit chain | no | yes (hash-chained Ed25519 log) |
| Eliza Action shape | yes | yes (same `name`/`similes`/`validate`/`handler`/`examples`) |

**Composable, not competitive.** Use Bleyle's plugin for the raw KH execution path and ours as the policy gate that decides whether the raw call should fire — drop ours upstream of his Action in the same character. Or use ours alone for the full gate-then-execute path: the SBO3L daemon ships a built-in KeeperHub adapter that runs the webhook on allow and surfaces the captured `executionId` as `kh_execution_ref`.

## Install

```bash
npm install @sbo3l/elizaos-keeperhub @sbo3l/sdk
```

## 5-line setup

```ts
import { SBO3LClient } from "@sbo3l/sdk";
import { sbo3lElizaKeeperHubAction } from "@sbo3l/elizaos-keeperhub";

const client = new SBO3LClient({ endpoint: "http://localhost:8730" });
const action = sbo3lElizaKeeperHubAction({ client });
// pass `action` into your character's plugin.actions[]
```

## Wire path

1. Action input: ElizaOS message containing an APRP — either `message.content.aprp` (object) or `message.content.text` (JSON-stringified APRP).
2. POST to SBO3L daemon `/v1/payment-requests`.
3. SBO3L decides allow / deny / requires_human against the loaded policy + budget + nonce + provider trust list.
4. On allow: SBO3L's `executor_callback` hands the signed `PolicyReceipt` to the daemon-side KeeperHub adapter (configured via `SBO3L_KEEPERHUB_WEBHOOK_URL` + `SBO3L_KEEPERHUB_TOKEN` env vars on the **daemon** process — not on the agent).
5. KH adapter POSTs the IP-1 envelope to the workflow webhook, captures `executionId`, surfaces it as `receipt.execution_ref`.
6. Action returns:
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

The `_advisory` suffix is intentional: today the daemon's env-configured webhook URL is the source of truth for actual routing. The per-call `workflowId` you pass to `sbo3lElizaKeeperHubAction({ workflowId })` is surfaced in the envelope for **context tagging** / audit logs, not as a routing override. See [KeeperHub/cli#52](https://github.com/KeeperHub/cli/issues/52) for the proposed contract that would make per-call routing safe.

## API

```ts
sbo3lElizaKeeperHubAction({
  client: SBO3LClientLike,
  workflowId?: string,             // default: DEFAULT_KH_WORKFLOW_ID
  name?: string,                   // default: "SBO3L_KEEPERHUB_PAYMENT_REQUEST"
  description?: string,
  idempotencyKey?: (aprp) => string,
  extractAprp?: (message) => object | null,
}): SBO3LElizaKHActionDescriptor
```

`SBO3LClientLike` = anything with `submit(request, opts?) → Promise<SBO3LSubmitResult>`. `@sbo3l/sdk`'s `SBO3LClient` matches nominally; mocks/fakes can implement just this for tests.

The returned descriptor is shape-compatible with `@elizaos/core`'s `Action` interface — drop directly into `plugin.actions[]` (or wrap in your own `ElizaPlugin`-shaped object).

## Composing with `@sbo3l/elizaos`

`@sbo3l/elizaos` ships the generic SBO3L Action (`SBO3L_PAYMENT_REQUEST` — destination-agnostic). This package ships a KH-specific variant (`SBO3L_KEEPERHUB_PAYMENT_REQUEST`) that surfaces the KH-specific `kh_execution_ref` + `kh_workflow_id_advisory` fields. Different action names — they don't conflict; use both in the same plugin if you want the agent to disambiguate by destination class.

## License

MIT
