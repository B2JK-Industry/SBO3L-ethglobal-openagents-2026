/**
 * `@sbo3l/elizaos-keeperhub` — ElizaOS Action that gates KeeperHub workflow
 * execution through SBO3L's policy boundary.
 *
 * # Why this exists alongside Bleyle's ElizaOS plugin
 *
 * Bleyle's ElizaOS plugin wraps **execution** — the agent emits a payment
 * intent and the plugin POSTs straight to KH. Our package gates execution
 * **upstream**: agent → SBO3L (policy + budget + audit + signed receipt) →
 * if allow → KH webhook → result. The two are **composable, not competitive**:
 * a developer can keep using Bleyle's plugin for the raw KH binding and drop
 * ours in front as the policy gate that decides whether the raw call should
 * fire at all. Or use ours alone for the full gate-then-execute path — the
 * SBO3L daemon ships a built-in KeeperHub adapter that runs the webhook on
 * allow and surfaces the captured `executionId` as `kh_execution_ref`.
 *
 * # The wire path
 *
 * 1. Action input: ElizaOS message containing an APRP (Agent Payment Request
 *    Protocol) — either `message.content.aprp` (object) or
 *    `message.content.text` (JSON-stringified).
 * 2. POST to SBO3L daemon's `/v1/payment-requests`.
 * 3. SBO3L decides allow / deny / requires_human against the loaded policy
 *    + budget + nonce + provider trust list.
 * 4. On allow: SBO3L's `executor_callback` hands the signed PolicyReceipt
 *    to the daemon-side KeeperHub adapter (`crates/sbo3l-keeperhub-adapter`,
 *    configured via `SBO3L_KEEPERHUB_WEBHOOK_URL` + `SBO3L_KEEPERHUB_TOKEN`
 *    env vars on the **daemon** process — not on the agent).
 * 5. KH adapter POSTs the IP-1 envelope to the workflow webhook, captures
 *    the `executionId`, surfaces it as `receipt.execution_ref`.
 * 6. Action returns:
 *    `{ decision, kh_workflow_id_advisory, kh_execution_ref, audit_event_id, request_hash, policy_hash, matched_rule_id, deny_code }`.
 *
 * # Two ways to consume
 *
 * Either as a structural Action descriptor (no `@elizaos/core` dep — its API
 * is in flux):
 *
 *   ```ts
 *   import { SBO3LClient } from "@sbo3l/sdk";
 *   import { sbo3lElizaKeeperHubAction } from "@sbo3l/elizaos-keeperhub";
 *
 *   const action = sbo3lElizaKeeperHubAction({
 *     client: new SBO3LClient({ endpoint: "http://localhost:8730" }),
 *   });
 *   // Pass `action` into your character's plugin.actions[] (or wrap in your
 *   // own ElizaPlugin-shaped object).
 *   ```
 *
 * Or wrapped into an ElizaOS-style Plugin object alongside the existing
 * `@sbo3l/elizaos` non-KH plugin (they don't conflict — different action
 * names).
 */

/** Live KeeperHub workflow id verified end-to-end on 2026-04-30. */
export const DEFAULT_KH_WORKFLOW_ID = "m4t4cnpmhv8qquce3bv3c";

/**
 * Minimum surface this Action needs from an SBO3L client. `@sbo3l/sdk`'s
 * `SBO3LClient` matches this nominally; mocks/fakes can implement just
 * this for tests.
 */
export interface SBO3LClientLike {
  submit(
    request: Record<string, unknown>,
    options?: { idempotencyKey?: string },
  ): Promise<SBO3LSubmitResult>;
}

/** Subset of the SBO3L response envelope this Action returns to the LLM. */
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
 * Minimal Eliza-shaped Action callback. The full Eliza signature includes a
 * `files?` parameter — we accept it but don't use it.
 */
export type ElizaCallback = (
  response: { text: string },
  files?: unknown[],
) => void;

export interface ElizaExample {
  user: string;
  content: { text: string; action?: string };
}

/**
 * Minimal Eliza-shaped Action. Mirrors `@elizaos/core`'s `Action` interface
 * (name + similes + description + examples + validate + handler) without
 * importing it.
 *
 * Eliza's runtime / message / state types are deliberately `unknown` here —
 * consumers cast at the boundary. Returning a string from the handler is
 * the convention (forwarded back to the LLM).
 */
export interface SBO3LElizaKHActionDescriptor {
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

export interface SBO3LElizaKHActionOptions {
  /** SBO3L client instance — anything matching `SBO3LClientLike`. */
  client: SBO3LClientLike;
  /**
   * Advisory KH workflow id surfaced in the envelope as
   * `kh_workflow_id_advisory`. Defaults to `DEFAULT_KH_WORKFLOW_ID`.
   * Note: the daemon's env-configured webhook URL is the source of
   * truth for actual routing — this value is for context tagging /
   * audit logs, not a per-call routing override. See README.
   */
  workflowId?: string;
  /** Override the default action name. */
  name?: string;
  /** Override the default action description. */
  description?: string;
  /** Optional callback to derive an idempotency key per call. */
  idempotencyKey?: (aprp: Record<string, unknown>) => string;
  /**
   * How to extract the APRP body from an Eliza `message`. Defaults to
   * looking for `message.content.aprp` (a JSON object) or
   * `message.content.text` (a JSON-stringified APRP).
   */
  extractAprp?: (message: unknown) => Record<string, unknown> | null;
}

const DEFAULT_NAME = "SBO3L_KEEPERHUB_PAYMENT_REQUEST";
const DEFAULT_DESCRIPTION =
  "Submit an Agent Payment Request Protocol (APRP) to SBO3L for policy " +
  "decision. On allow, the SBO3L daemon's KeeperHub adapter executes the " +
  "payment by POSTing the IP-1 envelope to a KeeperHub workflow webhook " +
  "and returns the captured executionId as kh_execution_ref. APRP MUST " +
  "appear in message.content.aprp (object) or message.content.text " +
  "(JSON-stringified). Returns: {decision, kh_workflow_id_advisory, " +
  "kh_execution_ref, audit_event_id, request_hash, policy_hash, " +
  "matched_rule_id, deny_code}. On deny, branch on deny_code to " +
  "self-correct or escalate.";

function defaultExtractAprp(
  message: unknown,
): Record<string, unknown> | null {
  if (typeof message !== "object" || message === null) return null;
  const m = message as { content?: unknown };
  if (typeof m.content !== "object" || m.content === null) return null;
  const c = m.content as { aprp?: unknown; text?: unknown };
  if (
    typeof c.aprp === "object" &&
    c.aprp !== null &&
    !Array.isArray(c.aprp)
  ) {
    return c.aprp as Record<string, unknown>;
  }
  if (typeof c.text === "string") {
    try {
      const parsed: unknown = JSON.parse(c.text);
      if (
        typeof parsed === "object" &&
        parsed !== null &&
        !Array.isArray(parsed)
      ) {
        return parsed as Record<string, unknown>;
      }
    } catch {
      return null;
    }
  }
  return null;
}

/**
 * Build the SBO3L → KeeperHub ElizaOS Action descriptor. Pass into your
 * character's plugin.actions[] (or wrap in your own ElizaPlugin-shaped
 * object).
 */
export function sbo3lElizaKeeperHubAction(
  options: SBO3LElizaKHActionOptions,
): SBO3LElizaKHActionDescriptor {
  const { client } = options;
  const name = options.name ?? DEFAULT_NAME;
  const description = options.description ?? DEFAULT_DESCRIPTION;
  const workflowId = options.workflowId ?? DEFAULT_KH_WORKFLOW_ID;
  const extract = options.extractAprp ?? defaultExtractAprp;

  return {
    name,
    similes: [
      "PAY_VIA_KEEPERHUB",
      "PURCHASE_VIA_KEEPERHUB",
      "SUBMIT_KH_PAYMENT",
      "REQUEST_KH_PAYMENT",
    ],
    description,
    examples: [
      {
        user: "agent",
        content: {
          text:
            "I need to pay 0.05 USDC for an inference call routed via " +
            "KeeperHub workflow.",
          action: name,
        },
      },
    ],
    validate: async (_runtime, message) => extract(message) !== null,
    handler: async (_runtime, message, _state, _options, callback) => {
      const aprp = extract(message);
      if (aprp === null) {
        const text = JSON.stringify({
          error: "input.no_aprp_in_message",
          detail:
            "could not extract an APRP object from message.content.aprp or .text",
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
        const executionRef =
          r.decision === "allow" ? r.receipt.execution_ref : null;
        envelope = {
          decision: r.decision,
          // `kh_workflow_id_advisory` (vs `kh_workflow_id`) is intentional:
          // the daemon's env-configured webhook URL is the source of truth
          // for actual routing. This value is for context tagging only.
          kh_workflow_id_advisory: workflowId,
          kh_execution_ref: executionRef,
          audit_event_id: r.audit_event_id,
          request_hash: r.request_hash,
          policy_hash: r.policy_hash,
          matched_rule_id: r.matched_rule_id,
          deny_code: r.deny_code,
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
}
