/**
 * `@sbo3l/langchain` — drop-in LangChain JS Tool that gates an agent's
 * payment intent through SBO3L's policy boundary.
 *
 * Pattern: agent emits a tool call with a JSON-stringified APRP → this Tool
 * forwards to `SBO3LClient.submit()` → returns the decision envelope as a
 * JSON string back to the agent. On `deny`, the agent sees the deny code
 * and can self-correct (or escalate). On `allow`, the agent sees the
 * `execution_ref` and can continue downstream.
 *
 * The integration uses **structural typing** for the SBO3L client: any
 * object with a `submit(request, options?)` method matching `SBO3LClientLike`
 * works. This means `@sbo3l/sdk` is a peer dep — install it separately.
 *
 * Usage:
 *   ```ts
 *   import { SBO3LClient } from "@sbo3l/sdk";
 *   import { sbo3lTool } from "@sbo3l/langchain";
 *
 *   const tool = sbo3lTool({
 *     client: new SBO3LClient({ endpoint: "http://localhost:8730" }),
 *   });
 *   // pass `tool` into your LangChain agent's tool list
 *   ```
 */

/**
 * Minimum surface this Tool needs from an SBO3L client. `@sbo3l/sdk`'s
 * `SBO3LClient` matches this nominally; mocks/fakes can implement just
 * this for tests.
 */
export interface SBO3LClientLike {
  submit(
    request: Record<string, unknown>,
    options?: { idempotencyKey?: string },
  ): Promise<SBO3LSubmitResult>;
}

/** Subset of the SBO3L response envelope this Tool returns to the LLM. */
export interface SBO3LSubmitResult {
  decision: "allow" | "deny" | "requires_human";
  deny_code: string | null;
  matched_rule_id: string | null;
  request_hash: string;
  policy_hash: string;
  audit_event_id: string;
  receipt: {
    execution_ref: string | null;
    [k: string]: unknown;
  };
}

/**
 * LangChain-compatible tool descriptor. We do not import from `langchain`
 * directly — that would make `langchain` a hard dep. Instead we expose
 * the structural shape `langchain` accepts (a plain object with `name`,
 * `description`, `schema`, and an async `func`).
 *
 * Wire it into LangChain via:
 *   `import { DynamicTool } from "@langchain/core/tools";`
 *   `const tool = new DynamicTool({ ...sbo3lTool({ client }), name: "...", description: "..." });`
 */
export interface SBO3LToolDescriptor {
  name: string;
  description: string;
  /**
   * Tool callback. Input is a JSON-stringified APRP. Returns a JSON string
   * containing the decision envelope (or an `error` field on transport
   * failure).
   */
  func: (input: string) => Promise<string>;
}

export interface SBO3LToolOptions {
  /** SBO3L client instance — anything matching `SBO3LClientLike`. */
  client: SBO3LClientLike;
  /** Override the default tool name. */
  name?: string;
  /** Override the default tool description. */
  description?: string;
  /** Optional callback to derive an idempotency key per call. */
  idempotencyKey?: (input: Record<string, unknown>) => string;
}

const DEFAULT_NAME = "sbo3l_payment_request";
const DEFAULT_DESCRIPTION =
  "Submit an Agent Payment Request Protocol (APRP) JSON object to SBO3L for policy decision. " +
  "Input MUST be a JSON-stringified APRP object containing fields: agent_id, task_id, intent, " +
  "amount, token, destination, payment_protocol, chain, provider_url, expiry, nonce, risk_class. " +
  "Returns a JSON object with decision (allow|deny|requires_human), execution_ref (when allowed), " +
  "and audit_event_id. On deny, branch on deny_code to self-correct or escalate.";

/**
 * Build the SBO3L LangChain tool descriptor. Pass into `DynamicTool` /
 * `DynamicStructuredTool` (or any LangChain tool factory) by spreading.
 */
export function sbo3lTool(options: SBO3LToolOptions): SBO3LToolDescriptor {
  const { client } = options;
  const name = options.name ?? DEFAULT_NAME;
  const description = options.description ?? DEFAULT_DESCRIPTION;

  return {
    name,
    description,
    func: async (input: string): Promise<string> => {
      let parsed: Record<string, unknown>;
      try {
        const v: unknown = JSON.parse(input);
        if (typeof v !== "object" || v === null || Array.isArray(v)) {
          return JSON.stringify({
            error: "input must be a JSON object (APRP)",
            input_received_type:
              v === null ? "null" : Array.isArray(v) ? "array" : typeof v,
          });
        }
        parsed = v as Record<string, unknown>;
      } catch (e) {
        return JSON.stringify({
          error: "input is not valid JSON",
          detail: e instanceof Error ? e.message : String(e),
        });
      }

      const submitOpts =
        options.idempotencyKey !== undefined
          ? { idempotencyKey: options.idempotencyKey(parsed) }
          : {};

      try {
        const r = await client.submit(parsed, submitOpts);
        return JSON.stringify({
          decision: r.decision,
          deny_code: r.deny_code,
          matched_rule_id: r.matched_rule_id,
          execution_ref: r.receipt.execution_ref,
          audit_event_id: r.audit_event_id,
          request_hash: r.request_hash,
          policy_hash: r.policy_hash,
        });
      } catch (e) {
        // Surface the SBO3L error code (RFC 7807) when present so the LLM
        // can branch on it; otherwise emit a generic transport failure.
        const code = (e as { code?: unknown })?.code;
        const status = (e as { status?: unknown })?.status;
        return JSON.stringify({
          error: typeof code === "string" ? code : "transport.failed",
          status: typeof status === "number" ? status : null,
          detail: e instanceof Error ? e.message : String(e),
        });
      }
    },
  };
}
