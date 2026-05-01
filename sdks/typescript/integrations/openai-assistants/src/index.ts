/**
 * `@sbo3l/openai-assistants` — OpenAI Assistants API adapter for SBO3L.
 *
 * Wraps `@sbo3l/sdk`'s `SBO3LClient.submit()` as an OpenAI Assistants
 * `function` tool definition. The Assistants API expects raw JSON Schema
 * for tool parameters (not zod / pydantic), so we hand-author the schema
 * mirroring `schemas/aprp_v1.json` directly — keeps the integration
 * dependency-free.
 *
 * Typical wiring:
 *
 *   ```ts
 *   import OpenAI from "openai";
 *   import { SBO3LClient } from "@sbo3l/sdk";
 *   import { sbo3lAssistantTool, runSbo3lToolCall } from "@sbo3l/openai-assistants";
 *
 *   const openai = new OpenAI();
 *   const client = new SBO3LClient({ endpoint: "http://localhost:8730" });
 *   const tool = sbo3lAssistantTool({ client });
 *
 *   const assistant = await openai.beta.assistants.create({
 *     model: "gpt-4o",
 *     tools: [tool.definition],
 *   });
 *
 *   // Inside your run-polling loop, when status === "requires_action":
 *   const outputs = await Promise.all(
 *     run.required_action.submit_tool_outputs.tool_calls.map((call) =>
 *       runSbo3lToolCall(tool, call),
 *     ),
 *   );
 *   await openai.beta.threads.runs.submitToolOutputs(thread.id, run.id, {
 *     tool_outputs: outputs,
 *   });
 *   ```
 */

import {
  SBO3LClient,
  SBO3LError,
  type PaymentRequest,
  type PolicyReceipt,
} from "@sbo3l/sdk";

export type { PolicyReceipt, PaymentRequest };

/**
 * Thrown when SBO3L returns `deny` / `requires_human`. The Assistants
 * runner catches this in [`runSbo3lToolCall`] and converts it into a
 * structured `tool_output` so the model can self-correct rather than
 * crashing the run.
 */
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
  "payment-shaped action. Returns a signed PolicyReceipt on allow, or a " +
  "structured deny envelope (with deny_code + audit_event_id) on deny so you can " +
  "self-correct or escalate.";

/**
 * APRP v1 JSON Schema for OpenAI tool `parameters`. Hand-authored from
 * `schemas/aprp_v1.json` so this package has no schema-generation dep.
 *
 * The shape exactly mirrors the SDK's `PaymentRequest` type — any drift
 * between this schema and the SDK is caught at compile time by the cast in
 * [`sbo3lAssistantTool`]'s `execute` (which assigns the parsed args to a
 * `PaymentRequest`).
 */
export const APRP_JSON_SCHEMA = {
  type: "object",
  additionalProperties: false,
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
    agent_id: {
      type: "string",
      pattern: "^[a-z0-9][a-z0-9_-]{2,63}$",
      description: "Stable agent slug (lowercase alphanumeric, _, -; 3-64 chars).",
    },
    task_id: {
      type: "string",
      pattern: "^[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$",
      description: "Caller-chosen task identifier (1-64 chars).",
    },
    intent: {
      type: "string",
      enum: [
        "purchase_api_call",
        "purchase_dataset",
        "pay_compute_job",
        "pay_agent_service",
        "tip",
      ],
      description: "What the agent intends to do with the payment.",
    },
    amount: {
      type: "object",
      additionalProperties: false,
      required: ["value", "currency"],
      properties: {
        value: {
          type: "string",
          pattern: "^(0|[1-9][0-9]*)(\\.[0-9]{1,18})?$",
          description: 'Decimal string (e.g. "0.05").',
        },
        currency: { type: "string", const: "USD" },
      },
      description: "Amount in fiat-pegged units.",
    },
    token: {
      type: "string",
      pattern: "^[A-Z0-9]{2,16}$",
      description: "Settlement token symbol (e.g. USDC, USDT).",
    },
    destination: {
      type: "object",
      // additionalProperties stays loose because the destination shape
      // depends on `type` (x402_endpoint vs eoa vs erc20_transfer).
      // The SDK validates the discriminated union server-side.
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
      description: "Where the payment goes; shape depends on `type`.",
    },
    payment_protocol: {
      type: "string",
      enum: ["x402", "l402", "erc20_transfer", "smart_account_session"],
    },
    chain: {
      type: "string",
      pattern: "^[a-z0-9][a-z0-9_-]{1,31}$",
      description: "Chain id (e.g. base, sepolia).",
    },
    provider_url: { type: "string", description: "Service provider URL (max 2048 chars)." },
    expiry: {
      type: "string",
      description: "RFC 3339 timestamp after which the request is invalid.",
    },
    nonce: {
      type: "string",
      pattern: "^[0-7][0-9A-HJKMNP-TV-Z]{25}$|^[0-9a-fA-F-]{8,}$",
      description: "ULID or UUID for replay protection.",
    },
    risk_class: { type: "string", enum: ["low", "medium", "high", "critical"] },
  },
} as const;

/** Shape of an OpenAI Assistants `function` tool definition (subset we emit). */
export interface AssistantFunctionTool {
  type: "function";
  function: {
    name: string;
    description: string;
    parameters: typeof APRP_JSON_SCHEMA;
  };
}

/** Shape of an OpenAI Assistants tool_call (subset we consume). */
export interface AssistantToolCall {
  id: string;
  type: "function";
  function: {
    name: string;
    arguments: string; // JSON-encoded
  };
}

/** Output that should be passed to `submitToolOutputs`. */
export interface AssistantToolOutput {
  tool_call_id: string;
  output: string; // JSON-encoded
}

export interface SBO3LToolOptions {
  client: SBO3LClient;
  /** Override tool name. Default: `sbo3l_payment_request`. Must match across calls. */
  name?: string;
  /** Override the description shown to the LLM. */
  description?: string;
  /** Optional callback to derive an idempotency key per call. */
  idempotencyKey?: (aprp: PaymentRequest) => string;
}

export interface SBO3LAssistantTool {
  /** The function-tool definition to pass into `assistants.create({ tools: [...] })`. */
  definition: AssistantFunctionTool;
  /** Tool name (matches `definition.function.name`). Use to dispatch tool_calls. */
  name: string;
  /**
   * Execute one parsed APRP through the SBO3L client. Throws
   * [`PolicyDenyError`] on `deny`/`requires_human`, returns the
   * `PolicyReceipt` on `allow`. Transport errors bubble as `SBO3LError`.
   */
  execute: (aprp: PaymentRequest) => Promise<PolicyReceipt>;
}

/**
 * Build the SBO3L Assistants tool. Pair with [`runSbo3lToolCall`] to
 * convert one `tool_call` into the `tool_output` the API expects.
 */
export function sbo3lAssistantTool(options: SBO3LToolOptions): SBO3LAssistantTool {
  const { client } = options;
  const name = options.name ?? DEFAULT_TOOL_NAME;
  const description = options.description ?? DEFAULT_DESCRIPTION;

  return {
    name,
    definition: {
      type: "function",
      function: { name, description, parameters: APRP_JSON_SCHEMA },
    },
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

/**
 * Handle one `tool_call` from a run's `required_action`. Parses the
 * function arguments, dispatches to the tool, and produces the
 * `submitToolOutputs` payload.
 *
 * On `allow` the output is the JSON-encoded `PolicyReceipt`. On any
 * exception (deny / transport / parse) the output is a JSON envelope the
 * model can branch on:
 *
 *   `{ "error": "policy.deny", "deny_code": "...", "audit_event_id": "...", "decision": "deny" }`
 *   `{ "error": "transport.failed", "detail": "..." }`
 *   `{ "error": "input.bad_arguments", "detail": "..." }`
 *
 * This deliberately does NOT re-throw — the run would otherwise be
 * cancelled by the API and the model never gets a chance to self-correct.
 */
export async function runSbo3lToolCall(
  tool: SBO3LAssistantTool,
  call: AssistantToolCall,
): Promise<AssistantToolOutput> {
  if (call.function.name !== tool.name) {
    return {
      tool_call_id: call.id,
      output: JSON.stringify({
        error: "input.unknown_tool",
        detail: `expected '${tool.name}', got '${call.function.name}'`,
      }),
    };
  }

  let args: PaymentRequest;
  try {
    args = JSON.parse(call.function.arguments) as PaymentRequest;
  } catch (e) {
    return {
      tool_call_id: call.id,
      output: JSON.stringify({
        error: "input.bad_arguments",
        detail: e instanceof Error ? e.message : String(e),
      }),
    };
  }

  try {
    const receipt = await tool.execute(args);
    return { tool_call_id: call.id, output: JSON.stringify(receipt) };
  } catch (e) {
    if (e instanceof PolicyDenyError) {
      return {
        tool_call_id: call.id,
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
        output: JSON.stringify({
          error: "transport.failed",
          detail: e.message,
        }),
      };
    }
    return {
      tool_call_id: call.id,
      output: JSON.stringify({
        error: "transport.unknown",
        detail: e instanceof Error ? e.message : String(e),
      }),
    };
  }
}

export { SBO3LError };
