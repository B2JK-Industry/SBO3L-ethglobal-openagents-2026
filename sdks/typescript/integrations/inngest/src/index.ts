/**
 * `@sbo3l/inngest` — durable-workflow adapter for SBO3L.
 *
 * Inngest is a TypeScript-native durable workflow runner: a function
 * is decomposed into named `step.run("name", () => ...)` calls, and
 * Inngest persists each step's result so a retry replays from the
 * last successful step.
 *
 * The SBO3L gate must run BEFORE any payment-shaped step. This adapter
 * supplies a `gateAprp(step, sbo3l, aprp)` helper that:
 *
 *   - wraps the SBO3L submit in a `step.run("sbo3l.submit", ...)` so
 *     the receipt is persisted alongside other step outputs (no double
 *     submit on retry — Inngest's idempotency takes over)
 *   - on allow → returns the receipt; the caller's next `step.run`
 *     does the real action with the receipt's audit_event_id captured
 *     in the workflow's persisted state
 *   - on deny → throws PolicyDenyError, which Inngest treats as a
 *     terminal step failure (no infinite retry on policy denies — they
 *     are deterministic, not transient)
 *
 *   ```ts
 *   import { Inngest, NonRetriableError } from "inngest";
 *   import { SBO3LClient } from "@sbo3l/sdk";
 *   import { gateAprp, PolicyDenyError } from "@sbo3l/inngest";
 *
 *   const inngest = new Inngest({ id: "agent-runner" });
 *   const sbo3l = new SBO3LClient({ endpoint: "http://sbo3l:8730" });
 *
 *   export const swap = inngest.createFunction(
 *     { id: "agent.swap" },
 *     { event: "agent/swap.requested" },
 *     async ({ event, step }) => {
 *       try {
 *         const receipt = await gateAprp(step, sbo3l, event.data.aprp);
 *         await step.run("execute-swap", () => doSwap(event.data, receipt));
 *       } catch (e) {
 *         if (e instanceof PolicyDenyError) {
 *           // Inngest treats throws as retries by default; wrap to skip retries.
 *           throw new NonRetriableError(e.message);
 *         }
 *         throw e;
 *       }
 *     },
 *   );
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

/**
 * The minimum surface from Inngest's `step` parameter we depend on.
 * Defining a structural type (rather than importing from `inngest`)
 * keeps the adapter testable without the full Inngest dep AND lets
 * consumers stub it in unit tests.
 */
export interface InngestStepLike {
  run: <T>(id: string, handler: () => Promise<T>) => Promise<T>;
}

export interface GateOptions {
  /** Override the step id (defaults to `sbo3l.submit:<task_id>`). */
  stepIdPrefix?: string;
  /** Optional callback to derive an idempotency key per call. */
  idempotencyKey?: (aprp: PaymentRequest) => string;
}

/**
 * Submit an APRP through SBO3L inside an Inngest workflow step. Uses
 * `step.run` so Inngest persists the receipt — on a workflow retry the
 * cached receipt is replayed instead of re-submitting (which would
 * trip `protocol.nonce_replay` deny).
 *
 * Throws `PolicyDenyError` on `deny`/`requires_human` so the caller
 * can wrap with `NonRetriableError` (Inngest convention).
 */
export async function gateAprp(
  step: InngestStepLike,
  sbo3l: SBO3LClient,
  aprp: PaymentRequest,
  options: GateOptions = {},
): Promise<PolicyReceipt> {
  const stepId = `${options.stepIdPrefix ?? "sbo3l.submit"}:${aprp.task_id}`;
  // The handler runs INSIDE Inngest's step machinery: its result is
  // serialised to the workflow journal so retries replay it. We return
  // a non-throwing union from the handler so Inngest persists the
  // deny envelope too (otherwise an exception inside step.run would
  // cause Inngest's retry loop to re-submit on every retry — dangerous,
  // because the same APRP would trip protocol.nonce_replay and deny
  // for the wrong reason).
  const envelope = await step.run(stepId, async () => {
    const submitOpts =
      options.idempotencyKey !== undefined
        ? { idempotencyKey: options.idempotencyKey(aprp) }
        : {};
    try {
      const r = await sbo3l.submit(aprp, submitOpts);
      return {
        kind: "decided" as const,
        decision: r.decision,
        deny_code: r.deny_code,
        matched_rule_id: r.matched_rule_id,
        audit_event_id: r.audit_event_id,
        receipt: r.receipt,
      };
    } catch (e) {
      // Transport errors (network down, daemon 5xx) ARE transient —
      // re-throw so Inngest retries the step. Same convention as
      // Inngest's defaults.
      if (e instanceof SBO3LError) throw e;
      throw e;
    }
  });

  if (envelope.decision !== "allow") {
    throw new PolicyDenyError(
      envelope.decision as "deny" | "requires_human",
      envelope.deny_code,
      envelope.matched_rule_id,
      envelope.audit_event_id,
    );
  }
  return envelope.receipt;
}

/**
 * Convenience wrapper that catches `PolicyDenyError` and surfaces it
 * as a structured step result (no throw). Use this when the workflow
 * has its own deny-handling branch and shouldn't fall through to
 * Inngest's retry / NonRetriable handling.
 */
export async function gateAprpSafe(
  step: InngestStepLike,
  sbo3l: SBO3LClient,
  aprp: PaymentRequest,
  options: GateOptions = {},
): Promise<
  | { ok: true; receipt: PolicyReceipt }
  | { ok: false; decision: "deny" | "requires_human"; deny_code: string | null; audit_event_id: string }
> {
  try {
    const receipt = await gateAprp(step, sbo3l, aprp, options);
    return { ok: true, receipt };
  } catch (e) {
    if (e instanceof PolicyDenyError) {
      return {
        ok: false,
        decision: e.decision,
        deny_code: e.denyCode,
        audit_event_id: e.auditEventId,
      };
    }
    throw e;
  }
}

export { SBO3LError };
