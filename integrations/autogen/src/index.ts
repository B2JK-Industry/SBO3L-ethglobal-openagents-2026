/**
 * `@sbo3l/autogen` — Microsoft AutoGen function adapter wrapping SBO3L.
 *
 * AutoGen agents call functions (OpenAI-style: name + description +
 * parameters JSON Schema). This package exposes one such function:
 * `sbo3l_payment_request`. Drop into a `ConversableAgent`'s function
 * registry to gate every payment intent through SBO3L's policy boundary.
 *
 * Structural typing (`SBO3LClientLike` interface) — install `@sbo3l/sdk`
 * separately as an optional peerDep.
 */

export interface SBO3LClientLike {
  submit(
    request: Record<string, unknown>,
    options?: { idempotencyKey?: string },
  ): Promise<SBO3LSubmitResult>;
}

export interface SBO3LSubmitResult {
  decision: "allow" | "deny" | "requires_human";
  deny_code: string | null;
  matched_rule_id: string | null;
  request_hash: string;
  policy_hash: string;
  audit_event_id: string;
  receipt: { execution_ref: string | null; [k: string]: unknown };
}

export interface AutoGenFunctionDescriptor {
  name: string;
  description: string;
  parameters: Record<string, unknown>;
  call: (args: Record<string, unknown>) => Promise<SBO3LFunctionResult>;
}

export interface SBO3LFunctionResult {
  decision?: "allow" | "deny" | "requires_human";
  deny_code?: string | null;
  matched_rule_id?: string | null;
  execution_ref?: string | null;
  audit_event_id?: string;
  request_hash?: string;
  policy_hash?: string;
  error?: string;
  status?: number | null;
  detail?: string;
}

export interface SBO3LFunctionOptions {
  client: SBO3LClientLike;
  name?: string;
  description?: string;
  idempotencyKey?: (args: Record<string, unknown>) => string;
}

const DEFAULT_NAME = "sbo3l_payment_request";
const DEFAULT_DESCRIPTION =
  "Submit an Agent Payment Request Protocol (APRP) to SBO3L for policy decision. " +
  "Returns decision (allow|deny|requires_human), execution_ref (when allowed), " +
  "and audit_event_id. On deny, branch on deny_code to self-correct or escalate.";

const APRP_PARAMETERS: Record<string, unknown> = {
  type: "object",
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
    agent_id: { type: "string" },
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
    destination: { type: "object" },
    payment_protocol: {
      type: "string",
      enum: ["x402", "l402", "erc20_transfer", "smart_account_session"],
    },
    chain: { type: "string" },
    provider_url: { type: "string" },
    expiry: { type: "string", description: "RFC 3339" },
    nonce: { type: "string", description: "ULID" },
    risk_class: { type: "string", enum: ["low", "medium", "high", "critical"] },
  },
  additionalProperties: true,
};

export function sbo3lFunction(options: SBO3LFunctionOptions): AutoGenFunctionDescriptor {
  const { client } = options;
  const name = options.name ?? DEFAULT_NAME;
  const description = options.description ?? DEFAULT_DESCRIPTION;

  return {
    name,
    description,
    parameters: APRP_PARAMETERS,
    call: async (args: Record<string, unknown>): Promise<SBO3LFunctionResult> => {
      const submitOpts =
        options.idempotencyKey !== undefined
          ? { idempotencyKey: options.idempotencyKey(args) }
          : {};

      try {
        const r = await client.submit(args, submitOpts);
        return {
          decision: r.decision,
          deny_code: r.deny_code,
          matched_rule_id: r.matched_rule_id,
          execution_ref: r.receipt.execution_ref,
          audit_event_id: r.audit_event_id,
          request_hash: r.request_hash,
          policy_hash: r.policy_hash,
        };
      } catch (e) {
        const code = (e as { code?: unknown })?.code;
        const status = (e as { status?: unknown })?.status;
        return {
          error: typeof code === "string" ? code : "transport.failed",
          status: typeof status === "number" ? status : null,
          detail: e instanceof Error ? e.message : String(e),
        };
      }
    },
  };
}
