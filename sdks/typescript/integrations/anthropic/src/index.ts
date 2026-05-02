/**
 * `@sbo3l/anthropic` — Anthropic tool-use adapter for SBO3L.
 *
 * Wraps `@sbo3l/sdk`'s `SBO3LClient.submit()` as a Tool definition
 * compatible with `anthropic.messages.create({ tools: [...] })`. The tool's
 * input shape is validated by zod (mirrors APRP v1) before reaching the
 * daemon, so malformed model output denies fast and locally.
 *
 * Typical wiring:
 *
 *   ```ts
 *   import Anthropic from "@anthropic-ai/sdk";
 *   import { SBO3LClient } from "@sbo3l/sdk";
 *   import { sbo3lTool, runSbo3lToolUse } from "@sbo3l/anthropic";
 *
 *   const claude = new Anthropic();
 *   const client = new SBO3LClient({ endpoint: "http://localhost:8730" });
 *   const tool = sbo3lTool({ client });
 *
 *   let messages = [{ role: "user", content: "Pay 0.05 USDC for an inference call." }];
 *   for (;;) {
 *     const r = await claude.messages.create({
 *       model: "claude-3-5-sonnet-latest",
 *       max_tokens: 1024,
 *       tools: [tool.definition],
 *       messages,
 *     });
 *     messages.push({ role: "assistant", content: r.content });
 *     if (r.stop_reason !== "tool_use") break;
 *     const toolUses = r.content.filter((c) => c.type === "tool_use");
 *     const results = await Promise.all(toolUses.map((u) => runSbo3lToolUse(tool, u)));
 *     messages.push({ role: "user", content: results });
 *   }
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

/** Thrown when SBO3L returns `deny` / `requires_human` and the caller uses `tool.execute()` directly. */
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
  "payment-shaped action. Returns a signed PolicyReceipt on allow, or a structured " +
  "deny envelope (with deny_code + audit_event_id) on deny so you can self-correct " +
  "or escalate.";

/**
 * Zod schema mirroring APRP v1. Used both for local validation
 * (catching malformed model output before the request hits the daemon)
 * and to drive the Anthropic Tool's `input_schema` JSON Schema.
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
  nonce: z.string().describe("ULID or UUID for replay protection."),
  risk_class: z.enum(["low", "medium", "high", "critical"]),
});

/**
 * APRP v1 JSON Schema for Anthropic Tool `input_schema`. Hand-authored
 * (rather than zod-to-json-schema) so this package has zero dev deps
 * beyond zod. The shape mirrors the SDK's `PaymentRequest` type.
 */
export const APRP_INPUT_SCHEMA = {
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
    task_id: { type: "string", description: "Caller-chosen task id." },
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
        value: { type: "string", description: 'Decimal string (e.g. "0.05").' },
        currency: { type: "string", enum: ["USD"] },
      },
    },
    token: { type: "string", description: "Settlement token symbol (e.g. USDC, USDT)." },
    destination: {
      type: "object",
      required: ["type"],
      properties: {
        type: {
          type: "string",
          enum: ["x402_endpoint", "eoa", "smart_account", "erc20_transfer"],
        },
        url: { type: "string" },
        method: {
          type: "string",
          enum: ["GET", "POST", "PUT", "PATCH", "DELETE"],
        },
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
    chain: { type: "string", description: "Chain id (e.g. base, sepolia)." },
    provider_url: { type: "string" },
    expiry: { type: "string", description: "RFC 3339 timestamp." },
    nonce: { type: "string", description: "ULID or UUID for replay protection." },
    risk_class: { type: "string", enum: ["low", "medium", "high", "critical"] },
  },
} as const;

/** Anthropic Tool definition shape (subset we emit). */
export interface AnthropicToolDefinition {
  name: string;
  description: string;
  input_schema: typeof APRP_INPUT_SCHEMA;
}

/** A single `tool_use` content block from a Claude response (subset we consume). */
export interface AnthropicToolUseBlock {
  type: "tool_use";
  id: string;
  name: string;
  input: unknown;
}

/** A `tool_result` content block to push back into the next message. */
export interface AnthropicToolResultBlock {
  type: "tool_result";
  tool_use_id: string;
  content: string;
  is_error?: boolean;
}

export interface SBO3LToolOptions {
  client: SBO3LClient;
  /** Override tool name. Default: `sbo3l_payment_request`. */
  name?: string;
  /** Override the description shown to the LLM. */
  description?: string;
  /** Optional callback to derive an idempotency key per call. */
  idempotencyKey?: (aprp: PaymentRequest) => string;
}

export interface SBO3LAnthropicTool {
  /** Tool definition to pass into `messages.create({ tools: [...] })`. */
  definition: AnthropicToolDefinition;
  /** Tool name (matches `definition.name`). Use to dispatch tool_use blocks. */
  name: string;
  /**
   * Execute one parsed APRP through the SBO3L client. Throws
   * [`PolicyDenyError`] on `deny`/`requires_human`, returns the
   * `PolicyReceipt` on `allow`. Validates the input shape against
   * `aprpSchema` before submitting; throws `ZodError` on bad input.
   */
  execute: (aprp: unknown) => Promise<PolicyReceipt>;
}

export function sbo3lTool(options: SBO3LToolOptions): SBO3LAnthropicTool {
  const { client } = options;
  const name = options.name ?? DEFAULT_TOOL_NAME;
  const description = options.description ?? DEFAULT_DESCRIPTION;

  return {
    name,
    definition: { name, description, input_schema: APRP_INPUT_SCHEMA },
    execute: async (raw): Promise<PolicyReceipt> => {
      // zod-validate before hitting the daemon. Catches malformed model
      // output (wrong enum value, missing field) without a network round
      // trip and produces a useful error the model can branch on.
      const aprp = aprpSchema.parse(raw) as PaymentRequest;
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

/**
 * Handle one `tool_use` content block from a Claude response. Dispatches
 * the input through the SBO3L tool and produces the `tool_result` block
 * to push into the next `messages.create` call.
 *
 * On exception this returns a `tool_result` with `is_error: true` and a
 * structured envelope so the model can self-correct mid-conversation
 * rather than crashing the loop:
 *
 *   - allow → `{ ...PolicyReceipt }` as the content
 *   - deny → `{ error: 'policy.deny', deny_code, audit_event_id }`
 *   - requires_human → `{ error: 'policy.requires_human', audit_event_id }`
 *   - bad input shape → `{ error: 'input.bad_arguments', issues: [...] }`
 *   - transport fail → `{ error: 'transport.failed', detail }`
 *   - unknown tool name → `{ error: 'input.unknown_tool', detail }`
 */
export async function runSbo3lToolUse(
  tool: SBO3LAnthropicTool,
  block: AnthropicToolUseBlock,
): Promise<AnthropicToolResultBlock> {
  if (block.name !== tool.name) {
    return {
      type: "tool_result",
      tool_use_id: block.id,
      is_error: true,
      content: JSON.stringify({
        error: "input.unknown_tool",
        detail: `expected '${tool.name}', got '${block.name}'`,
      }),
    };
  }

  try {
    const receipt = await tool.execute(block.input);
    return {
      type: "tool_result",
      tool_use_id: block.id,
      content: JSON.stringify(receipt),
    };
  } catch (e) {
    if (e instanceof PolicyDenyError) {
      return {
        type: "tool_result",
        tool_use_id: block.id,
        is_error: true,
        content: JSON.stringify({
          error: e.decision === "deny" ? "policy.deny" : "policy.requires_human",
          decision: e.decision,
          deny_code: e.denyCode,
          matched_rule_id: e.matchedRuleId,
          audit_event_id: e.auditEventId,
        }),
      };
    }
    if (e instanceof z.ZodError) {
      return {
        type: "tool_result",
        tool_use_id: block.id,
        is_error: true,
        content: JSON.stringify({
          error: "input.bad_arguments",
          issues: e.issues.map((i) => ({ path: i.path, message: i.message })),
        }),
      };
    }
    if (e instanceof SBO3LError) {
      return {
        type: "tool_result",
        tool_use_id: block.id,
        is_error: true,
        content: JSON.stringify({ error: "transport.failed", detail: e.message }),
      };
    }
    return {
      type: "tool_result",
      tool_use_id: block.id,
      is_error: true,
      content: JSON.stringify({
        error: "transport.unknown",
        detail: e instanceof Error ? e.message : String(e),
      }),
    };
  }
}

export { SBO3LError };
