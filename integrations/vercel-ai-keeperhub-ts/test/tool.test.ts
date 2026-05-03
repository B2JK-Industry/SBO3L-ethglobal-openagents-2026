import { describe, expect, it, vi } from "vitest";
import {
  DEFAULT_KH_WORKFLOW_ID,
  sbo3lVercelAIKeeperHubTool,
  type SBO3LClientLike,
  type SBO3LSubmitResult,
} from "../src/index.js";

const ALLOW_RESPONSE: SBO3LSubmitResult = {
  decision: "allow",
  deny_code: null,
  matched_rule_id: "allow-low-risk-x402-keeperhub",
  request_hash: "c0bd2fab".repeat(8),
  policy_hash: "e044f13c".repeat(8),
  audit_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGR",
  receipt: {
    execution_ref: "kh-01HTAWX5K3R8YV9NQB7C6P2DGZ",
  },
};

const DENY_RESPONSE: SBO3LSubmitResult = {
  decision: "deny",
  deny_code: "policy.amount_over_limit",
  matched_rule_id: "deny-high-amount",
  request_hash: "deadbeef".repeat(8),
  policy_hash: "cafebabe".repeat(8),
  audit_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGS",
  receipt: {
    execution_ref: null,
  },
};

const APRP: Record<string, unknown> = {
  agent_id: "research-agent-01",
  task_id: "kh-test-1",
  intent: "purchase_api_call",
};

describe("sbo3lVercelAIKeeperHubTool", () => {
  it("returns an ai.tool() descriptor with parameters + execute", () => {
    const t = sbo3lVercelAIKeeperHubTool({
      client: { submit: vi.fn() } as unknown as SBO3LClientLike,
    });
    expect(typeof t.description).toBe("string");
    expect(t.description).toContain("KeeperHub");
    expect(t.description).toContain("kh_execution_ref");
    expect(t.parameters).toBeDefined();
    expect(typeof t.execute).toBe("function");
  });

  it("surfaces kh_execution_ref + advisory workflow id on allow (envelope is object, not JSON string)", async () => {
    const submit = vi.fn().mockResolvedValue(ALLOW_RESPONSE);
    const t = sbo3lVercelAIKeeperHubTool({ client: { submit } });
    const out = await t.execute!({ aprp: APRP }, {} as never);

    // CRITICAL: out is a plain object, NOT a JSON-stringified string —
    // Vercel AI SDK lets the tool return objects directly.
    expect(typeof out).toBe("object");
    expect(typeof out).not.toBe("string");

    expect((out as { decision: string }).decision).toBe("allow");
    expect((out as { kh_execution_ref: string }).kh_execution_ref).toBe(
      "kh-01HTAWX5K3R8YV9NQB7C6P2DGZ",
    );
    expect(
      (out as { kh_workflow_id_advisory: string }).kh_workflow_id_advisory,
    ).toBe(DEFAULT_KH_WORKFLOW_ID);
    expect((out as { audit_event_id: string }).audit_event_id).toBe(
      "evt-01HTAWX5K3R8YV9NQB7C6P2DGR",
    );
    expect((out as { deny_code: string | null }).deny_code).toBeNull();
    expect(submit).toHaveBeenCalledOnce();
  });

  it("does NOT surface kh_execution_ref on deny", async () => {
    const submit = vi.fn().mockResolvedValue(DENY_RESPONSE);
    const t = sbo3lVercelAIKeeperHubTool({ client: { submit } });
    const out = await t.execute!({ aprp: APRP }, {} as never);

    expect((out as { decision: string }).decision).toBe("deny");
    expect((out as { kh_execution_ref: string | null }).kh_execution_ref).toBeNull();
    expect((out as { deny_code: string }).deny_code).toBe(
      "policy.amount_over_limit",
    );
    // Advisory workflow id is still surfaced so the agent / audit log
    // knows which workflow was *intended* even though execution didn't happen.
    expect(
      (out as { kh_workflow_id_advisory: string }).kh_workflow_id_advisory,
    ).toBe(DEFAULT_KH_WORKFLOW_ID);
  });

  it("honors workflowId override", async () => {
    const submit = vi.fn().mockResolvedValue(ALLOW_RESPONSE);
    const t = sbo3lVercelAIKeeperHubTool({
      client: { submit },
      workflowId: "kh-staging-workflow-xyz",
    });
    const out = await t.execute!({ aprp: APRP }, {} as never);

    expect(
      (out as { kh_workflow_id_advisory: string }).kh_workflow_id_advisory,
    ).toBe("kh-staging-workflow-xyz");
  });

  it("rejects invalid input via the zod parameters schema (LLM contract)", () => {
    // The AI SDK validates `parameters` BEFORE calling execute; we exercise
    // the schema directly to prove the contract — `aprp` is required and
    // must be an object (not a string, not missing).
    const t = sbo3lVercelAIKeeperHubTool({
      client: { submit: vi.fn() } as unknown as SBO3LClientLike,
    });
    const r1 = t.parameters.safeParse({});
    expect(r1.success).toBe(false); // missing aprp
    const r2 = t.parameters.safeParse({ aprp: "not-an-object" });
    expect(r2.success).toBe(false); // aprp must be record
    const r3 = t.parameters.safeParse({ aprp: { agent_id: "research-agent-01" } });
    expect(r3.success).toBe(true); // record-of-unknown is permissive
  });

  it("returns structured error envelope on transport failure with code", async () => {
    const err: { code: string; status: number; message: string } = {
      code: "auth.required",
      status: 401,
      message: "missing bearer",
    };
    const submit = vi.fn().mockRejectedValue(err);
    const t = sbo3lVercelAIKeeperHubTool({ client: { submit } });
    const out = await t.execute!({ aprp: APRP }, {} as never);

    expect((out as { error: string }).error).toBe("auth.required");
    expect((out as { status: number }).status).toBe(401);
  });

  it("falls back to transport.failed on opaque exception", async () => {
    const submit = vi.fn().mockRejectedValue(new Error("network down"));
    const t = sbo3lVercelAIKeeperHubTool({ client: { submit } });
    const out = await t.execute!({ aprp: APRP }, {} as never);

    expect((out as { error: string }).error).toBe("transport.failed");
    expect((out as { detail: string }).detail).toContain("network down");
  });

  it("calls idempotencyKey callback when provided", async () => {
    const submit = vi.fn().mockResolvedValue(ALLOW_RESPONSE);
    const idempotencyKey = vi.fn().mockReturnValue("idem-key-xyz");
    const t = sbo3lVercelAIKeeperHubTool({
      client: { submit },
      idempotencyKey,
    });
    await t.execute!({ aprp: APRP }, {} as never);

    expect(idempotencyKey).toHaveBeenCalledOnce();
    expect(submit).toHaveBeenCalledWith(expect.any(Object), {
      idempotencyKey: "idem-key-xyz",
    });
  });

  it("envelope is a plain object on allow (NOT a JSON-stringified string)", async () => {
    const submit = vi.fn().mockResolvedValue(ALLOW_RESPONSE);
    const t = sbo3lVercelAIKeeperHubTool({ client: { submit } });
    const out = await t.execute!({ aprp: APRP }, {} as never);

    // This is the load-bearing contract for Vercel AI SDK: tool().execute()
    // returns a value the LLM sees as the tool result. Returning an object
    // (not a string) means the LLM gets structured fields it can branch on
    // without re-parsing.
    expect(out).toBeTypeOf("object");
    expect(out).not.toBeNull();
    expect(Array.isArray(out)).toBe(false);
    // Sanity: structured fields are real properties on the object, not
    // characters of a string.
    expect((out as { decision: string }).decision).toBe("allow");
    expect((out as { audit_event_id: string }).audit_event_id).toMatch(
      /^evt-/,
    );
  });
});
