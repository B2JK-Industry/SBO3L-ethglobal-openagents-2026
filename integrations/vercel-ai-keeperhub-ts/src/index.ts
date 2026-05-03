/**
 * `@sbo3l/vercel-ai-keeperhub` — Vercel AI SDK tool that gates KeeperHub
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
 * # Why a Vercel-AI-flavored variant
 *
 * Vercel AI SDK is the fastest-growing TypeScript agent framework and the
 * de-facto choice for Edge-runtime / Next.js Route Handlers. Plugs in as
 * an `ai.tool()` directly into `streamText` / `generateText`'s `tools`
 * map — the LLM gets a typed `parameters` (zod) and the tool result is a
 * plain JS object the LLM can branch on.
 *
 * # The wire path
 *
 * 1. Tool input (LLM-supplied): `{ aprp: { ... } }` — APRP body as a typed
 *    object (NOT a JSON-stringified string — Vercel AI SDK's zod
 *    parameters mean the LLM sees a structured input).
 * 2. POST to SBO3L daemon's `/v1/payment-requests`.
 * 3. SBO3L decides allow / deny / requires_human against the loaded
 *    policy + budget + nonce + provider trust list.
 * 4. On allow: SBO3L's `executor_callback` hands the signed PolicyReceipt
 *    to the daemon-side KeeperHub adapter (`crates/sbo3l-keeperhub-adapter`,
 *    configured via `SBO3L_KEEPERHUB_WEBHOOK_URL` + `SBO3L_KEEPERHUB_TOKEN`
 *    env vars on the **daemon** process — not on the agent).
 * 5. KH adapter POSTs the IP-1 envelope to the workflow webhook, captures
 *    the `executionId`, surfaces it as `receipt.execution_ref`.
 * 6. Tool returns (as an object, not JSON-stringified):
 *    `{ decision, kh_workflow_id_advisory, kh_execution_ref, audit_event_id, request_hash, policy_hash, matched_rule_id, deny_code }`.
 *
 * # Usage
 *
 *   ```ts
 *   import { streamText } from "ai";
 *   import { openai } from "@ai-sdk/openai";
 *   import { SBO3LClient } from "@sbo3l/sdk";
 *   import { sbo3lVercelAIKeeperHubTool } from "@sbo3l/vercel-ai-keeperhub";
 *
 *   const client = new SBO3LClient({ endpoint: "http://localhost:8730" });
 *
 *   const result = streamText({
 *     model: openai("gpt-4o"),
 *     tools: { sbo3lKeeperHub: sbo3lVercelAIKeeperHubTool({ client }) },
 *     prompt: "Pay 0.05 USDC for an inference call via the KeeperHub workflow.",
 *   });
 *   ```
 */

import { tool } from "ai";
import { z } from "zod";

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

/** Shape of the envelope returned to the LLM as the tool result. */
export interface SBO3LKeeperHubEnvelope {
  decision: "allow" | "deny" | "requires_human";
  /**
   * `_advisory` (vs `kh_workflow_id`) is intentional honest naming: today
   * the daemon's env-configured webhook URL is the source of truth for
   * actual routing. The per-call value is for context tagging / audit
   * logs, not a routing override.
   */
  kh_workflow_id_advisory: string;
  kh_execution_ref: string | null;
  audit_event_id: string;
  request_hash: string;
  policy_hash: string;
  matched_rule_id: string | null;
  deny_code: string | null;
}

/** Shape returned on a transport failure (object, not thrown). */
export interface SBO3LKeeperHubErrorEnvelope {
  error: string;
  status: number | null;
  detail: string;
}

export interface SBO3LVercelAIKeeperHubToolOptions {
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
  /** Override the default tool description shown to the LLM. */
  description?: string;
  /** Optional callback to derive an idempotency key per call. */
  idempotencyKey?: (input: Record<string, unknown>) => string;
}

const DEFAULT_DESCRIPTION =
  "Submit an Agent Payment Request Protocol (APRP) object to SBO3L for " +
  "policy decision. On allow, the SBO3L daemon's KeeperHub adapter executes " +
  "the payment by POSTing the IP-1 envelope to a KeeperHub workflow webhook " +
  "and returns the captured executionId as kh_execution_ref. Returns: " +
  "{decision, kh_workflow_id_advisory, kh_execution_ref, audit_event_id, " +
  "request_hash, policy_hash, deny_code}. On deny, branch on deny_code to " +
  "self-correct or escalate.";

/**
 * Zod parameters schema. The LLM sees a typed `aprp` object parameter
 * (record of unknown values) — not a JSON-stringified blob. The SBO3L
 * daemon validates the APRP against `schemas/aprp_v1.json` server-side,
 * so we keep the LLM-facing schema permissive (record-of-unknown) and
 * defer the strict shape check to the daemon. This avoids re-encoding
 * the full APRP zod schema here (it lives in `@sbo3l/vercel-ai`).
 */
const parametersSchema = z.object({
  aprp: z
    .record(z.unknown())
    .describe(
      "APRP (Agent Payment Request Protocol) v1 body as an object. Required " +
        "fields: agent_id, task_id, intent, amount, token, destination, " +
        "payment_protocol, chain, provider_url, expiry, nonce, risk_class. " +
        "See https://schemas.sbo3l.dev/aprp_v1.json for the full schema. " +
        "The SBO3L daemon validates this server-side.",
    ),
});

/**
 * Build the SBO3L → KeeperHub Vercel AI tool. Pass into a `streamText` /
 * `generateText`'s `tools` map.
 *
 * Returns an `ai.tool()` descriptor with `{ description, parameters, execute }`.
 * `execute` returns the envelope as a plain object (not JSON-stringified) —
 * the LLM sees structured fields it can branch on directly.
 *
 * On transport failure: returns an error envelope object
 * `{ error, status, detail }` (does NOT throw — keeps the tool result
 * stream-friendly and lets the LLM self-correct).
 */
export function sbo3lVercelAIKeeperHubTool(
  options: SBO3LVercelAIKeeperHubToolOptions,
) {
  const { client } = options;
  const description = options.description ?? DEFAULT_DESCRIPTION;
  const workflowId = options.workflowId ?? DEFAULT_KH_WORKFLOW_ID;

  return tool({
    description,
    parameters: parametersSchema,
    execute: async (
      args: { aprp: Record<string, unknown> },
    ): Promise<SBO3LKeeperHubEnvelope | SBO3LKeeperHubErrorEnvelope> => {
      const parsed = args.aprp;

      const submitOpts =
        options.idempotencyKey !== undefined
          ? { idempotencyKey: options.idempotencyKey(parsed) }
          : {};

      try {
        const r = await client.submit(parsed, submitOpts);
        const executionRef =
          r.decision === "allow" ? r.receipt.execution_ref : null;

        return {
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
        return {
          error: typeof code === "string" ? code : "transport.failed",
          status: typeof status === "number" ? status : null,
          detail: e instanceof Error ? e.message : String(e),
        };
      }
    },
  });
}
