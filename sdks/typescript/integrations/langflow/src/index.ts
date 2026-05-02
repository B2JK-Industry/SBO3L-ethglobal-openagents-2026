/**
 * `@sbo3l/langflow` — LangFlow tool component adapter for SBO3L.
 *
 * LangFlow's runtime accepts custom tool components that expose a JSON
 * descriptor (`{ name, description, inputs, outputs, build }`). The
 * descriptor is registered in LangFlow Studio's component registry and
 * the visual flow editor can drop it into any agent flow.
 *
 * This adapter emits the descriptor wired to APRP v1 + a `build()`
 * function the LangFlow runtime calls when a tool node fires. On allow
 * the build function returns the signed PolicyReceipt; on deny it
 * returns a structured envelope so the upstream LLM node can branch.
 *
 *   ```ts
 *   import { SBO3LClient } from "@sbo3l/sdk";
 *   import { sbo3lLangFlowComponent } from "@sbo3l/langflow";
 *
 *   const sbo3l = new SBO3LClient({ endpoint: "http://localhost:8730" });
 *   const component = sbo3lLangFlowComponent({ client: sbo3l });
 *
 *   // Hand `component.descriptor` to LangFlow's component registry,
 *   // and `component.build` becomes the runtime callable for this tool.
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

export const DEFAULT_COMPONENT_NAME = "sbo3l_payment_request";

const DEFAULT_DESCRIPTION =
  "LangFlow component: submit a payment intent through SBO3L's policy boundary " +
  "BEFORE attempting any payment-shaped action. Returns the signed PolicyReceipt on " +
  "allow, or a structured envelope on deny so the flow can branch.";

/**
 * APRP v1 input schema for the LangFlow component's `inputs` field.
 * Hand-authored from `schemas/aprp_v1.json` — no schema-gen dep.
 */
export const APRP_INPUTS_SCHEMA = {
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
    expiry: { type: "string", description: "RFC 3339 timestamp." },
    nonce: { type: "string", description: "ULID or UUID." },
    risk_class: { type: "string", enum: ["low", "medium", "high", "critical"] },
  },
} as const;

export const APRP_OUTPUTS_SCHEMA = {
  type: "object" as const,
  description: "PolicyReceipt on allow, or { error, deny_code, audit_event_id } on deny.",
} as const;

/** LangFlow component descriptor (subset). */
export interface LangFlowComponentDescriptor {
  name: string;
  description: string;
  inputs: typeof APRP_INPUTS_SCHEMA;
  outputs: typeof APRP_OUTPUTS_SCHEMA;
}

/** LangFlow's runtime-shaped output envelope. */
export type LangFlowComponentOutput =
  | { ok: true; data: PolicyReceipt; audit_event_id: string }
  | { ok: false; error: string; deny_code: string | null; audit_event_id: string | null };

export interface SBO3LLangFlowComponent {
  descriptor: LangFlowComponentDescriptor;
  name: string;
  /**
   * Runtime callable. LangFlow invokes this with the parsed APRP input;
   * returns the structured envelope LangFlow surfaces to the next node.
   * Never re-throws — denies + transport errors both surface as
   * `{ ok: false, ... }` so the flow can branch.
   */
  build: (aprp: PaymentRequest) => Promise<LangFlowComponentOutput>;
}

export interface SBO3LComponentOptions {
  client: SBO3LClient;
  name?: string;
  description?: string;
  idempotencyKey?: (aprp: PaymentRequest) => string;
}

export function sbo3lLangFlowComponent(
  options: SBO3LComponentOptions,
): SBO3LLangFlowComponent {
  const { client } = options;
  const name = options.name ?? DEFAULT_COMPONENT_NAME;
  const description = options.description ?? DEFAULT_DESCRIPTION;

  return {
    name,
    descriptor: {
      name,
      description,
      inputs: APRP_INPUTS_SCHEMA,
      outputs: APRP_OUTPUTS_SCHEMA,
    },
    build: async (aprp): Promise<LangFlowComponentOutput> => {
      const submitOpts =
        options.idempotencyKey !== undefined
          ? { idempotencyKey: options.idempotencyKey(aprp) }
          : {};
      try {
        const r = await client.submit(aprp, submitOpts);
        if (r.decision !== "allow") {
          return {
            ok: false,
            error: r.decision === "deny" ? "policy.deny" : "policy.requires_human",
            deny_code: r.deny_code,
            audit_event_id: r.audit_event_id,
          };
        }
        return { ok: true, data: r.receipt, audit_event_id: r.audit_event_id };
      } catch (e) {
        if (e instanceof SBO3LError) {
          return {
            ok: false,
            error: "transport.failed",
            deny_code: null,
            audit_event_id: null,
          };
        }
        return {
          ok: false,
          error: "transport.unknown",
          deny_code: null,
          audit_event_id: null,
        };
      }
    },
  };
}

export { SBO3LError };
