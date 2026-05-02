/**
 * `@sbo3l/together` — SBO3L adapter for the Together agent / API surface.
 *
 * Wraps `@sbo3l/sdk`'s `SBO3LClient.submit()` in a tool descriptor +
 * a runner helper that converts framework tool calls into the standard
 * SBO3L allow / deny / requires_human envelope. Same pattern as the
 * 18 prior adapters in this monorepo.
 *
 *   ```ts
 *   import { SBO3LClient } from "@sbo3l/sdk";
 *   import { sbo3lTogetherTool, runSbo3lTogetherToolCall } from "@sbo3l/together";
 *
 *   const sbo3l = new SBO3LClient({ endpoint: "http://localhost:8730" });
 *   const tool = sbo3lTogetherTool({ client: sbo3l });
 *
 *   // Wire `tool.descriptor` into Together's tool registry; pair
 *   // each emitted tool call with `runSbo3lTogetherToolCall`.
 *   ```
 */

import {
  SBO3LClient,
  SBO3LError,
  type PaymentRequest,
  type PolicyReceipt,
} from "@sbo3l/sdk";

export type { PaymentRequest, PolicyReceipt };

export class PolicyDenyError extends Error {
  override readonly name = "PolicyDenyError";

  readonly decision: "deny" | "requires_human";
  readonly denyCode: string | null;
  readonly matchedRuleId: string | null;
  readonly auditEventId: string;

  constructor(
    decision: "deny" | "requires_human",
    denyCode: string | null,
    matchedRuleId: string | null,
    auditEventId: string,
  ) {
    super(
      decision === "deny"
        ? `SBO3L denied payment intent (${denyCode ?? "policy.unknown"})`
        : `SBO3L requires human approval for payment intent (${denyCode ?? "policy.requires_human"})`,
    );
    this.decision = decision;
    this.denyCode = denyCode;
    this.matchedRuleId = matchedRuleId;
    this.auditEventId = auditEventId;
  }
}

export const DEFAULT_TOOL_NAME = "sbo3l_payment_request";

const DEFAULT_DESCRIPTION =
  "Submit a payment intent through SBO3L's policy boundary BEFORE attempting any " +
  "payment-shaped action. Returns the signed PolicyReceipt on allow, or a structured " +
  "deny envelope on deny so the agent can self-correct.";

/** APRP v1 input schema for Together's tool descriptor. Hand-authored — no schema-gen dep. */
export const APRP_SCHEMA = {
  type: "object" as const,
  required: [
    "agent_id",
    "task_id",
    "intent",
    "amount",
    "token",
    "destination",
    "payment_protocol",
    "chain",
    "provider_url",
    "expiry",
    "nonce",
    "risk_class",
  ],
  properties: {
    agent_id: { type: "string", description: "Stable agent slug." },
    task_id: { type: "string" },
    intent: {
      type: "string",
      enum: [
        "purchase_api_call",
        "purchase_dataset",
        "pay_compute_job",
        "pay_agent_service",
        "tip",
      ],
    },
    amount: {
      type: "object",
      required: ["value", "currency"],
      properties: {
        value: { type: "string" },
        currency: { type: "string", enum: ["USD"] },
      },
    },
    token: { type: "string" },
    destination: {
      type: "object",
      required: ["type"],
      properties: {
        type: {
          type: "string",
          enum: ["x402_endpoint", "eoa", "smart_account", "erc20_transfer"],
        },
        url: { type: "string" },
        method: { type: "string", enum: ["GET", "POST", "PUT", "PATCH", "DELETE"] },
        address: { type: "string" },
        token_address: { type: "string" },
        recipient: { type: "string" },
        expected_recipient: { type: ["string", "null"] },
      },
    },
    payment_protocol: {
      type: "string",
      enum: ["x402", "l402", "erc20_transfer", "smart_account_session"],
    },
    chain: { type: "string" },
    provider_url: { type: "string" },
    expiry: { type: "string" },
    nonce: { type: "string" },
    risk_class: { type: "string", enum: ["low", "medium", "high", "critical"] },
  },
} as const;

export interface TogetherDescriptor {
  name: string;
  description: string;
  schema: typeof APRP_SCHEMA;
}

export interface TogetherToolCall {
  id: string;
  name: string;
  /** JSON-encoded APRP. */
  input: string;
}

export interface TogetherToolResult {
  tool_call_id: string;
  ok: boolean;
  /** JSON-encoded result body (PolicyReceipt on allow; deny envelope otherwise). */
  output: string;
}

export interface SBO3LToolOptions {
  client: SBO3LClient;
  name?: string;
  description?: string;
  idempotencyKey?: (aprp: PaymentRequest) => string;
}

export interface SBO3LTogetherTool {
  descriptor: TogetherDescriptor;
  name: string;
  /** Throws `PolicyDenyError` on deny / requires_human, returns receipt on allow. */
  execute: (aprp: PaymentRequest) => Promise<PolicyReceipt>;
}

export function sbo3lTogetherTool(options: SBO3LToolOptions): SBO3LTogetherTool {
  const { client } = options;
  const name = options.name ?? DEFAULT_TOOL_NAME;
  const description = options.description ?? DEFAULT_DESCRIPTION;

  return {
    name,
    descriptor: { name, description, schema: APRP_SCHEMA },
    execute: async (aprp): Promise<PolicyReceipt> => {
      const submitOpts =
        options.idempotencyKey !== undefined
          ? { idempotencyKey: options.idempotencyKey(aprp) }
          : {};
      const r = await client.submit(aprp, submitOpts);
      if (r.decision !== "allow") {
        throw new PolicyDenyError(
          r.decision,
          r.deny_code,
          r.matched_rule_id,
          r.audit_event_id,
        );
      }
      return r.receipt;
    },
  };
}

export async function runSbo3lTogetherToolCall(
  tool: SBO3LTogetherTool,
  call: TogetherToolCall,
): Promise<TogetherToolResult> {
  if (call.name !== tool.name) {
    return {
      tool_call_id: call.id,
      ok: false,
      output: JSON.stringify({
        error: "input.unknown_tool",
        detail: `expected '${tool.name}', got '${call.name}'`,
      }),
    };
  }

  let args: PaymentRequest;
  try {
    args = JSON.parse(call.input) as PaymentRequest;
  } catch (e) {
    return {
      tool_call_id: call.id,
      ok: false,
      output: JSON.stringify({
        error: "input.bad_arguments",
        detail: e instanceof Error ? e.message : String(e),
      }),
    };
  }

  try {
    const receipt = await tool.execute(args);
    return {
      tool_call_id: call.id,
      ok: true,
      output: JSON.stringify(receipt),
    };
  } catch (e) {
    if (e instanceof PolicyDenyError) {
      return {
        tool_call_id: call.id,
        ok: false,
        output: JSON.stringify({
          error: e.decision === "deny" ? "policy.deny" : "policy.requires_human",
          decision: e.decision,
          deny_code: e.denyCode,
          matched_rule_id: e.matchedRuleId,
          audit_event_id: e.auditEventId,
        }),
      };
    }
    if (e instanceof SBO3LError) {
      return {
        tool_call_id: call.id,
        ok: false,
        output: JSON.stringify({ error: "transport.failed", detail: e.message }),
      };
    }
    return {
      tool_call_id: call.id,
      ok: false,
      output: JSON.stringify({
        error: "transport.unknown",
        detail: e instanceof Error ? e.message : String(e),
      }),
    };
  }
}

export { SBO3LError };
