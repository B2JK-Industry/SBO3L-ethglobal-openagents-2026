/**
 * `@sbo3l/mastra` — Mastra adapter for SBO3L.
 *
 * Wraps `@sbo3l/sdk`'s `SBO3LClient.submit()` as a Mastra Tool descriptor
 * with zod input + output schemas. The descriptor plugs into Mastra's
 * `Agent({ tools: { sbo3l_payment_request: ... } })` shape — Mastra
 * itself stays an optional peer dep so the integration can be tested
 * (and ship calldata-only smoke demos) without pulling in `@mastra/core`.
 *
 * Typical wiring:
 *
 *   ```ts
 *   import { Agent } from "@mastra/core/agent";
 *   import { openai } from "@ai-sdk/openai";
 *   import { SBO3LClient } from "@sbo3l/sdk";
 *   import { sbo3lTool } from "@sbo3l/mastra";
 *
 *   const client = new SBO3LClient({ endpoint: "http://localhost:8730" });
 *   const agent = new Agent({
 *     name: "research-agent",
 *     model: openai("gpt-4o"),
 *     tools: { sbo3l_payment_request: sbo3lTool({ client }) },
 *   });
 *
 *   const r = await agent.generate(
 *     "Pay 0.05 USDC for an inference call against api.example.com.",
 *   );
 *   ```
 */

import { z } from "zod";
import {
  SBO3LClient,
  SBO3LError,
  type PaymentRequest,
  type PolicyReceipt,
} from "@sbo3l/sdk";

export type { PolicyReceipt, PaymentRequest };

/** Thrown when SBO3L returns `deny` / `requires_human`. Surfaces in Mastra as a tool execution error. */
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

export const DEFAULT_TOOL_ID = "sbo3l_payment_request";

const DEFAULT_DESCRIPTION =
  "Submit a payment intent through SBO3L's policy boundary BEFORE attempting any " +
  "payment-shaped action. Returns the signed PolicyReceipt on allow, or throws " +
  "PolicyDenyError on deny so the agent can self-correct or escalate.";

/**
 * APRP v1 input schema (zod). Mastra renders this directly into the
 * tool's input contract; mirrors `schemas/aprp_v1.json`.
 */
export const inputSchema = z.object({
  agent_id: z
    .string()
    .regex(/^[a-z0-9][a-z0-9_-]{2,63}$/)
    .describe("Stable agent slug (lowercase alphanumeric, _, -; 3-64 chars)."),
  task_id: z
    .string()
    .regex(/^[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$/)
    .describe("Caller-chosen task identifier (1-64 chars)."),
  intent: z
    .enum([
      "purchase_api_call",
      "purchase_dataset",
      "pay_compute_job",
      "pay_agent_service",
      "tip",
    ])
    .describe("What the agent intends to do with the payment."),
  amount: z.object({
    value: z.string().regex(/^(0|[1-9][0-9]*)(\.[0-9]{1,18})?$/),
    currency: z.literal("USD"),
  }),
  token: z.string().regex(/^[A-Z0-9]{2,16}$/),
  destination: z.object({
    type: z.enum(["x402_endpoint", "eoa", "smart_account", "erc20_transfer"]),
    url: z.string().optional(),
    method: z.enum(["GET", "POST", "PUT", "PATCH", "DELETE"]).optional(),
    address: z.string().optional(),
    token_address: z.string().optional(),
    recipient: z.string().optional(),
    expected_recipient: z.string().nullable().optional(),
  }),
  payment_protocol: z.enum(["x402", "l402", "erc20_transfer", "smart_account_session"]),
  chain: z.string().regex(/^[a-z0-9][a-z0-9_-]{1,31}$/),
  provider_url: z.string(),
  expiry: z.string().describe("RFC 3339 timestamp."),
  nonce: z.string().describe("ULID or UUID for replay protection."),
  risk_class: z.enum(["low", "medium", "high", "critical"]),
});

/** Mastra-shaped output: subset of PolicyReceipt that the LLM most cares about. */
export const outputSchema = z.object({
  decision: z.literal("allow"),
  audit_event_id: z.string(),
  execution_ref: z.string().nullable(),
  receipt: z.unknown().describe("Full signed PolicyReceipt — pass to verify_capsule for offline check."),
});

export type Sbo3lToolInput = z.infer<typeof inputSchema>;
export type Sbo3lToolOutput = z.infer<typeof outputSchema>;

/** Mastra Tool descriptor (subset we emit). Compatible with `Agent({ tools })`. */
export interface MastraToolDescriptor {
  id: string;
  description: string;
  inputSchema: typeof inputSchema;
  outputSchema: typeof outputSchema;
  execute: (args: { context: Sbo3lToolInput }) => Promise<Sbo3lToolOutput>;
}

export interface SBO3LToolOptions {
  client: SBO3LClient;
  /** Override tool id. Default: `sbo3l_payment_request`. */
  id?: string;
  /** Override description shown to the agent. */
  description?: string;
  /** Optional callback to derive an idempotency key per call. */
  idempotencyKey?: (aprp: PaymentRequest) => string;
}

/**
 * Build the SBO3L Mastra tool. Plug into `new Agent({ tools: { [tool.id]: tool } })`.
 *
 * On `allow`: returns `{ decision, audit_event_id, execution_ref, receipt }`.
 * On `deny`/`requires_human`: throws [`PolicyDenyError`]. Mastra surfaces
 * this to the LLM as a tool-execution error so it can branch.
 */
export function sbo3lTool(options: SBO3LToolOptions): MastraToolDescriptor {
  const { client } = options;
  const id = options.id ?? DEFAULT_TOOL_ID;
  const description = options.description ?? DEFAULT_DESCRIPTION;

  return {
    id,
    description,
    inputSchema,
    outputSchema,
    execute: async ({ context }): Promise<Sbo3lToolOutput> => {
      const submitOpts =
        options.idempotencyKey !== undefined
          ? { idempotencyKey: options.idempotencyKey(context as PaymentRequest) }
          : {};
      const r = await client.submit(context as PaymentRequest, submitOpts);
      if (r.decision !== "allow") {
        throw new PolicyDenyError(
          r.decision,
          r.deny_code,
          r.matched_rule_id,
          r.audit_event_id,
        );
      }
      return {
        decision: "allow",
        audit_event_id: r.audit_event_id,
        execution_ref: r.receipt.execution_ref ?? null,
        receipt: r.receipt,
      };
    },
  };
}

export { SBO3LError };
