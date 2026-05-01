/**
 * `@sbo3l/elizaos` — ElizaOS plugin wrapping SBO3L.
 *
 * ElizaOS plugins export a `Plugin` object with `name`, `description`,
 * and `actions[]`. Each `Action` has a `name`, `validate(runtime, message)`,
 * and `handler(runtime, message, state, options, callback)`. This package
 * exposes one such Action: `SBO3L_PAYMENT_REQUEST`. Drop the plugin into
 * your character's plugin list to gate every payment intent through SBO3L.
 *
 * Structural typing — no hard import of `@elizaos/core` (its API is in
 * flux). Consumers can wrap `sbo3lPlugin(...)` into their preferred
 * Eliza Plugin shape, or use the exported types as-is if they match.
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

/**
 * Minimal Eliza-shaped Action. Mirrors `@elizaos/core`'s `Action` interface
 * (name + similes + description + validate + handler) without importing it.
 *
 * Eliza's runtime / message / state types are deliberately `unknown` here —
 * consumers cast at the boundary. Returning a string from the handler is
 * the convention (forwarded back to the LLM).
 */
export interface ElizaAction {
  name: string;
  similes: string[];
  description: string;
  examples: ElizaExample[];
  validate: (runtime: unknown, message: unknown) => Promise<boolean>;
  handler: (
    runtime: unknown,
    message: unknown,
    state?: unknown,
    options?: unknown,
    callback?: ElizaCallback,
  ) => Promise<string>;
}

export type ElizaCallback = (response: { text: string }, files?: unknown[]) => void;

export interface ElizaExample {
  user: string;
  content: { text: string; action?: string };
}

export interface ElizaPlugin {
  name: string;
  description: string;
  actions: ElizaAction[];
  evaluators: unknown[];
  providers: unknown[];
}

export interface SBO3LPluginOptions {
  client: SBO3LClientLike;
  /** Override the default action name. */
  actionName?: string;
  /** Optional callback to derive an idempotency key per call. */
  idempotencyKey?: (aprp: Record<string, unknown>) => string;
  /**
   * How to extract the APRP body from an Eliza `message`. Defaults to
   * looking for `message.content.aprp` (a JSON object) or
   * `message.content.text` (a JSON-stringified APRP).
   */
  extractAprp?: (message: unknown) => Record<string, unknown> | null;
}

const DEFAULT_ACTION_NAME = "SBO3L_PAYMENT_REQUEST";
const DEFAULT_DESCRIPTION =
  "Submit an Agent Payment Request Protocol (APRP) to SBO3L for policy decision. " +
  "Returns the decision (allow|deny|requires_human), execution_ref (when allowed), " +
  "and audit_event_id. On deny, branch on deny_code to self-correct or escalate.";

function defaultExtractAprp(message: unknown): Record<string, unknown> | null {
  if (typeof message !== "object" || message === null) return null;
  const m = message as { content?: unknown };
  if (typeof m.content !== "object" || m.content === null) return null;
  const c = m.content as { aprp?: unknown; text?: unknown };
  if (typeof c.aprp === "object" && c.aprp !== null && !Array.isArray(c.aprp)) {
    return c.aprp as Record<string, unknown>;
  }
  if (typeof c.text === "string") {
    try {
      const parsed: unknown = JSON.parse(c.text);
      if (typeof parsed === "object" && parsed !== null && !Array.isArray(parsed)) {
        return parsed as Record<string, unknown>;
      }
    } catch {
      return null;
    }
  }
  return null;
}

/**
 * Build the SBO3L Eliza plugin. Pass into your character's `plugins: [...]`
 * (or a Plugin loader that accepts `ElizaPlugin`-shaped objects).
 */
export function sbo3lPlugin(options: SBO3LPluginOptions): ElizaPlugin {
  const { client } = options;
  const actionName = options.actionName ?? DEFAULT_ACTION_NAME;
  const extract = options.extractAprp ?? defaultExtractAprp;

  const action: ElizaAction = {
    name: actionName,
    similes: ["PAY", "PURCHASE", "SUBMIT_PAYMENT", "REQUEST_PAYMENT"],
    description: DEFAULT_DESCRIPTION,
    examples: [
      {
        user: "agent",
        content: {
          text: "I need to pay 0.05 USDC for an inference call.",
          action: actionName,
        },
      },
    ],
    validate: async (_runtime, message) => extract(message) !== null,
    handler: async (_runtime, message, _state, _options, callback) => {
      const aprp = extract(message);
      if (aprp === null) {
        const text = JSON.stringify({
          error: "input.no_aprp_in_message",
          detail: "could not extract an APRP object from message.content.aprp or .text",
        });
        callback?.({ text });
        return text;
      }

      const submitOpts =
        options.idempotencyKey !== undefined
          ? { idempotencyKey: options.idempotencyKey(aprp) }
          : {};

      let envelope: Record<string, unknown>;
      try {
        const r = await client.submit(aprp, submitOpts);
        envelope = {
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
        envelope = {
          error: typeof code === "string" ? code : "transport.failed",
          status: typeof status === "number" ? status : null,
          detail: e instanceof Error ? e.message : String(e),
        };
      }

      const text = JSON.stringify(envelope);
      callback?.({ text });
      return text;
    },
  };

  return {
    name: "@sbo3l/elizaos",
    description:
      "SBO3L payment-request gate for ElizaOS — every payment intent passes through SBO3L's policy boundary.",
    actions: [action],
    evaluators: [],
    providers: [],
  };
}
