/**
 * `@sbo3l/langchain-keeperhub` — LangChain JS Tool that gates KeeperHub
 * workflow execution through SBO3L's policy boundary.
 *
 * # Why this exists alongside `langchain-keeperhub` (Devendra's npm/PyPI pkg)
 *
 * Devendra's `langchain-keeperhub` wraps **execution** — agent → KH webhook
 * → result. Our package gates execution **upstream**: agent → SBO3L
 * (policy + budget + audit + signed receipt) → if allow → KH webhook →
 * result. The two are **composable**: a developer can use Devendra's tool
 * for the raw KH binding and ours as the policy gate that decides whether
 * the raw call should fire at all. Or use ours alone for the full
 * gate-then-execute path.
 *
 * # The wire path
 *
 * 1. Tool input: JSON-stringified APRP (Agent Payment Request Protocol).
 * 2. POST to SBO3L daemon's `/v1/payment-requests`.
 * 3. SBO3L decides allow / deny / requires_human against the loaded
 *    policy + budget + nonce + provider trust list.
 * 4. On allow: SBO3L's `executor_callback` hands the signed PolicyReceipt
 *    to the daemon-side KeeperHub adapter (`crates/sbo3l-keeperhub-adapter`,
 *    configured via `SBO3L_KEEPERHUB_WEBHOOK_URL` + `SBO3L_KEEPERHUB_TOKEN`
 *    env vars on the **daemon** process — not on the agent).
 * 5. KH adapter POSTs the IP-1 envelope to the workflow webhook, captures
 *    the `executionId`, surfaces it as `receipt.execution_ref`.
 * 6. Tool returns:
 *    `{ decision, kh_workflow_id_advisory, kh_execution_ref, audit_event_id, request_hash, policy_hash, deny_code }`.
 *
 * # Two ways to consume
 *
 * Either as a structural descriptor (no LangChain dep — same shape as
 * `@sbo3l/langchain`):
 *
 *   ```ts
 *   import { SBO3LClient } from "@sbo3l/sdk";
 *   import { sbo3lKeeperHubTool } from "@sbo3l/langchain-keeperhub";
 *
 *   const tool = sbo3lKeeperHubTool({
 *     client: new SBO3LClient({ endpoint: "http://localhost:8730" }),
 *   });
 *   // wire into LangChain via DynamicTool / DynamicStructuredTool
 *   ```
 *
 * Or as a typed `StructuredTool` wrapper (when `@langchain/core` is a
 * runtime dep of your agent — we don't pull it in):
 *
 *   ```ts
 *   import { DynamicTool } from "@langchain/core/tools";
 *   import { sbo3lKeeperHubTool } from "@sbo3l/langchain-keeperhub";
 *
 *   const desc = sbo3lKeeperHubTool({ client });
 *   const tool = new DynamicTool({
 *     name: desc.name,
 *     description: desc.description,
 *     func: desc.func,
 *   });
 *   ```
 */

/** Live KeeperHub workflow id verified end-to-end on 2026-04-30. */
export const DEFAULT_KH_WORKFLOW_ID = "m4t4cnpmhv8qquce3bv3c";

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

export interface SBO3LKeeperHubToolDescriptor {
  name: string;
  description: string;
  /**
   * Tool callback. Input is a JSON-stringified APRP. Returns a JSON
   * string containing the gate-then-execute envelope (or an `error`
   * field on transport failure).
   */
  func: (input: string) => Promise<string>;
}

export interface SBO3LKeeperHubToolOptions {
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
  /** Override the default tool name. */
  name?: string;
  /** Override the default tool description. */
  description?: string;
  /** Optional callback to derive an idempotency key per call. */
  idempotencyKey?: (input: Record<string, unknown>) => string;
}

const DEFAULT_NAME = "sbo3l_keeperhub_payment_request";
const DEFAULT_DESCRIPTION =
  "Submit an Agent Payment Request Protocol (APRP) JSON object to SBO3L for " +
  "policy decision. On allow, the SBO3L daemon's KeeperHub adapter executes " +
  "the payment by POSTing the IP-1 envelope to a KeeperHub workflow webhook " +
  "and returns the captured executionId as kh_execution_ref. Input MUST be a " +
  "JSON-stringified APRP. Returns: {decision, kh_workflow_id_advisory, " +
  "kh_execution_ref, audit_event_id, request_hash, policy_hash, deny_code}. " +
  "On deny, branch on deny_code to self-correct or escalate.";

/**
 * Build the SBO3L → KeeperHub LangChain tool descriptor. Pass into
 * `DynamicTool` / `DynamicStructuredTool` (or any LangChain tool factory)
 * by spreading.
 */
export function sbo3lKeeperHubTool(
  options: SBO3LKeeperHubToolOptions,
): SBO3LKeeperHubToolDescriptor {
  const { client } = options;
  const name = options.name ?? DEFAULT_NAME;
  const description = options.description ?? DEFAULT_DESCRIPTION;
  const workflowId = options.workflowId ?? DEFAULT_KH_WORKFLOW_ID;

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
        const executionRef =
          r.decision === "allow" ? r.receipt.execution_ref : null;

        return JSON.stringify({
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
        });
      } catch (e) {
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
