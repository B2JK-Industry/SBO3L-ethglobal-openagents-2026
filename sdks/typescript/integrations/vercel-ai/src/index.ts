/**
 * `@sbo3l/vercel-ai` — Vercel AI SDK adapter for SBO3L.
 *
 * Wraps `@sbo3l/sdk`'s `SBO3LClient.submit()` as an `ai.tool()` with a zod
 * parameter schema mirroring APRP v1. Returns the full PolicyReceipt shape
 * on `allow`, throws `PolicyDenyError` on `deny`/`requires_human` so the
 * LLM's `streamText`/`generateText` callbacks can branch.
 *
 *   ```ts
 *   import { streamText } from "ai";
 *   import { openai } from "@ai-sdk/openai";
 *   import { SBO3LClient } from "@sbo3l/sdk";
 *   import { sbo3lTool } from "@sbo3l/vercel-ai";
 *
 *   const client = new SBO3LClient({ endpoint: "http://localhost:8730" });
 *
 *   const result = streamText({
 *     model: openai("gpt-4o"),
 *     tools: { pay: sbo3lTool({ client }) },
 *     prompt: "Pay 0.05 USDC for an inference call.",
 *   });
 *   ```
 */

import { tool } from "ai";
import { z } from "zod";
import { SBO3LClient, SBO3LError, type PolicyReceipt } from "@sbo3l/sdk";

/** Re-export so consumers don't need a separate `@sbo3l/sdk` import to type-check the result. */
export type { PolicyReceipt };

/** Thrown when SBO3L returns `deny` (or `requires_human`). The LLM sees this via tool execution failure. */
export class PolicyDenyError extends Error {
  override readonly name = "PolicyDenyError";

  /** RFC 7807 domain code for the deny reason (e.g. `policy.budget_exceeded`). May be null. */
  readonly denyCode: string | null;

  /** Decision: `deny` or `requires_human` — `allow` decisions never throw. */
  readonly decision: "deny" | "requires_human";

  /** Domain-rule id that matched, if any. */
  readonly matchedRuleId: string | null;

  /** Audit event id for the rejected decision. */
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

/**
 * Zod schema mirroring APRP v1 (`schemas/aprp_v1.json`). Each field carries
 * a `.describe(...)` so the LLM understands what to fill in.
 */
export const aprpSchema = z.object({
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
  amount: z
    .object({
      value: z
        .string()
        .regex(/^(0|[1-9][0-9]*)(\.[0-9]{1,18})?$/)
        .describe("Decimal string (e.g. \"0.05\")."),
      currency: z.literal("USD"),
    })
    .describe("Amount in fiat-pegged units."),
  token: z
    .string()
    .regex(/^[A-Z0-9]{2,16}$/)
    .describe("Settlement token symbol (e.g. USDC, USDT)."),
  destination: z
    .object({
      type: z.enum(["x402_endpoint", "eoa", "smart_account", "erc20_transfer"]),
      url: z.string().optional(),
      method: z.enum(["GET", "POST", "PUT", "PATCH", "DELETE"]).optional(),
      address: z.string().optional(),
      token_address: z.string().optional(),
      recipient: z.string().optional(),
      expected_recipient: z.string().nullable().optional(),
    })
    .describe("Where the payment goes; shape depends on `type`."),
  payment_protocol: z.enum(["x402", "l402", "erc20_transfer", "smart_account_session"]),
  chain: z.string().regex(/^[a-z0-9][a-z0-9_-]{1,31}$/).describe("Chain id (e.g. base, sepolia)."),
  provider_url: z.string().describe("Service provider URL (max 2048 chars)."),
  expiry: z.string().describe("RFC 3339 timestamp after which the request is invalid."),
  nonce: z
    .string()
    .regex(/^[0-7][0-9A-HJKMNP-TV-Z]{25}$/)
    .describe("ULID for replay protection."),
  risk_class: z.enum(["low", "medium", "high", "critical"]),
});

export interface SBO3LToolOptions {
  /** SBO3L client instance. Required — no implicit constructor. */
  client: SBO3LClient;
  /** Override the default tool description shown to the LLM. */
  description?: string;
  /** Optional callback to derive an idempotency key per call. */
  idempotencyKey?: (aprp: z.infer<typeof aprpSchema>) => string;
}

const DEFAULT_DESCRIPTION =
  "Submit a payment intent through SBO3L's policy boundary. Returns the signed " +
  "PolicyReceipt (decision=allow), or throws PolicyDenyError if the policy denies " +
  "the request. Always call this BEFORE attempting any payment-shaped action.";

/**
 * Build the SBO3L Vercel AI tool. Pass into a `streamText`/`generateText`'s
 * `tools` map, e.g. `tools: { pay: sbo3lTool({ client }) }`.
 *
 * On `allow`: returns the full `PolicyReceipt` (audit_event_id, execution_ref,
 * signature, etc.). The LLM sees this as the tool result.
 * On `deny`/`requires_human`: throws `PolicyDenyError`. The LLM sees this as
 * a tool execution error and can self-correct or escalate.
 *
 * Transport / auth failures bubble up as the SDK's `SBO3LError` /
 * `SBO3LTransportError`.
 */
export function sbo3lTool(options: SBO3LToolOptions) {
  const { client } = options;
  const description = options.description ?? DEFAULT_DESCRIPTION;

  return tool({
    description,
    parameters: aprpSchema,
    execute: async (args): Promise<PolicyReceipt> => {
      const submitOpts =
        options.idempotencyKey !== undefined
          ? { idempotencyKey: options.idempotencyKey(args) }
          : {};

      // Re-shape zod-parsed args back to wire shape (zod allows optional
      // fields that the wire format requires to be absent vs null).
      // The aprpSchema mirrors APRP v1; the SDK accepts the same shape.
      const r = await client.submit(args as Parameters<typeof client.submit>[0], submitOpts);

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
  });
}

/**
 * Re-export `SBO3LError` so consumers can `instanceof`-discriminate
 * transport errors from policy denials without a separate import.
 */
export { SBO3LError };
