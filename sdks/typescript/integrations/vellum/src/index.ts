/**
 * `@sbo3l/vellum` — Vellum AI adapter for SBO3L.
 *
 * Vellum's prompt + workflow runtime accepts function tools defined as
 * `{ name, description, parameters }` (JSON Schema). This adapter emits
 * a function-tool definition wired to APRP v1, plus a runner helper that
 * dispatches a single Vellum function-call into the SBO3L pipeline and
 * returns the result envelope Vellum expects.
 *
 * Typical wiring:
 *
 *   ```ts
 *   import { Vellum } from "vellum-ai";
 *   import { SBO3LClient } from "@sbo3l/sdk";
 *   import { sbo3lTool, runSbo3lFunctionCall } from "@sbo3l/vellum";
 *
 *   const vellum = new Vellum({ apiKey: process.env.VELLUM_API_KEY });
 *   const client = new SBO3LClient({ endpoint: "http://localhost:8730" });
 *   const tool = sbo3lTool({ client });
 *
 *   // Pass `tool.definition` into your Prompt's `function_tools` definitions
 *   // in Vellum Studio, or include it in workflow node config that exposes
 *   // function tools.
 *
 *   // When Vellum's runtime emits a function call, dispatch via the runner:
 *   const result = await runSbo3lFunctionCall(tool, {
 *     name: "sbo3l_payment_request",
 *     arguments: rawJsonString,
 *   });
 *   ```
 */

import {
  SBO3LClient,
  SBO3LError,
  type PaymentRequest,
  type PolicyReceipt,
} from "@sbo3l/sdk";

export type { PaymentRequest, PolicyReceipt };

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
  "deny envelope (with deny_code + audit_event_id) on deny so the workflow can " +
  "branch and self-correct.";

/**
 * APRP v1 JSON Schema for Vellum's `function_tools[].parameters`.
 * Hand-authored from `schemas/aprp_v1.json` so this package has zero
 * schema-generation deps.
 */
export const APRP_PARAMETERS_SCHEMA = {
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
    task_id: { type: "string", description: "Caller-chosen task identifier." },
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
        value: { type: "string", description: 'Decimal (e.g. "0.05").' },
        currency: { type: "string", enum: ["USD"] },
      },
    },
    token: { type: "string", description: "Settlement token symbol." },
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
    chain: { type: "string" },
    provider_url: { type: "string" },
    expiry: { type: "string", description: "RFC 3339 timestamp." },
    nonce: { type: "string", description: "ULID or UUID." },
    risk_class: { type: "string", enum: ["low", "medium", "high", "critical"] },
  },
} as const;

/** Function-tool definition shape (subset of Vellum's). */
export interface VellumFunctionTool {
  name: string;
  description: string;
  parameters: typeof APRP_PARAMETERS_SCHEMA;
}

/** Single function call emitted by Vellum at runtime. */
export interface VellumFunctionCall {
  name: string;
  /** JSON-encoded arguments string. */
  arguments: string;
}

/** Envelope to return as the function call's output. */
export interface VellumFunctionResult {
  name: string;
  /** JSON-encoded result string. */
  output: string;
  is_error: boolean;
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

export interface SBO3LVellumTool {
  /** Function-tool definition to register with the Vellum prompt / workflow. */
  definition: VellumFunctionTool;
  /** Tool name (matches `definition.name`). Use to dispatch function calls. */
  name: string;
  /**
   * Execute one parsed APRP through the SBO3L client. Throws
   * [`PolicyDenyError`] on `deny`/`requires_human`, returns the
   * `PolicyReceipt` on `allow`. Transport errors bubble as `SBO3LError`.
   */
  execute: (aprp: PaymentRequest) => Promise<PolicyReceipt>;
}

export function sbo3lTool(options: SBO3LToolOptions): SBO3LVellumTool {
  const { client } = options;
  const name = options.name ?? DEFAULT_TOOL_NAME;
  const description = options.description ?? DEFAULT_DESCRIPTION;

  return {
    name,
    definition: { name, description, parameters: APRP_PARAMETERS_SCHEMA },
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
 * Handle one function call emitted by Vellum. Parses the arguments,
 * dispatches to the tool, and produces the `VellumFunctionResult`
 * envelope. **Never re-throws** — the workflow continues on a deny so
 * the LLM can branch on the structured envelope rather than crashing
 * the run.
 */
export async function runSbo3lFunctionCall(
  tool: SBO3LVellumTool,
  call: VellumFunctionCall,
): Promise<VellumFunctionResult> {
  if (call.name !== tool.name) {
    return {
      name: call.name,
      is_error: true,
      output: JSON.stringify({
        error: "input.unknown_tool",
        detail: `expected '${tool.name}', got '${call.name}'`,
      }),
    };
  }

  let args: PaymentRequest;
  try {
    args = JSON.parse(call.arguments) as PaymentRequest;
  } catch (e) {
    return {
      name: call.name,
      is_error: true,
      output: JSON.stringify({
        error: "input.bad_arguments",
        detail: e instanceof Error ? e.message : String(e),
      }),
    };
  }

  try {
    const receipt = await tool.execute(args);
    return {
      name: call.name,
      is_error: false,
      output: JSON.stringify(receipt),
    };
  } catch (e) {
    if (e instanceof PolicyDenyError) {
      return {
        name: call.name,
        is_error: true,
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
        name: call.name,
        is_error: true,
        output: JSON.stringify({ error: "transport.failed", detail: e.message }),
      };
    }
    return {
      name: call.name,
      is_error: true,
      output: JSON.stringify({
        error: "transport.unknown",
        detail: e instanceof Error ? e.message : String(e),
      }),
    };
  }
}

export { SBO3LError };
