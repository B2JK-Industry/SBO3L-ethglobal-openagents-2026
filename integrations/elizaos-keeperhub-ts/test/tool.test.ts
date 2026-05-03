import { describe, expect, it, vi } from "vitest";
import {
  DEFAULT_KH_WORKFLOW_ID,
  sbo3lElizaKeeperHubAction,
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

const APRP = {
  agent_id: "research-agent-kh-01",
  task_id: "kh-test-1",
  intent: "purchase_api_call",
};

const APRP_MESSAGE = { content: { aprp: APRP } };

describe("sbo3lElizaKeeperHubAction", () => {
  it("returns Action descriptor with KH-flavored defaults", () => {
    const desc = sbo3lElizaKeeperHubAction({
      client: { submit: vi.fn() } as unknown as SBO3LClientLike,
    });
    expect(desc.name).toBe("SBO3L_KEEPERHUB_PAYMENT_REQUEST");
    expect(desc.description).toContain("KeeperHub");
    expect(desc.description).toContain("kh_execution_ref");
    expect(desc.similes).toContain("PAY_VIA_KEEPERHUB");
    expect(desc.examples).toHaveLength(1);
  });

  it("surfaces kh_execution_ref + advisory workflow id on allow", async () => {
    const submit = vi.fn().mockResolvedValue(ALLOW_RESPONSE);
    const desc = sbo3lElizaKeeperHubAction({ client: { submit } });
    const out = JSON.parse(await desc.handler({}, APRP_MESSAGE));

    expect(out.decision).toBe("allow");
    expect(out.kh_execution_ref).toBe("kh-01HTAWX5K3R8YV9NQB7C6P2DGZ");
    expect(out.kh_workflow_id_advisory).toBe(DEFAULT_KH_WORKFLOW_ID);
    expect(out.audit_event_id).toBe("evt-01HTAWX5K3R8YV9NQB7C6P2DGR");
    expect(out.deny_code).toBeNull();
    expect(submit).toHaveBeenCalledOnce();
  });

  it("does NOT surface kh_execution_ref on deny", async () => {
    const submit = vi.fn().mockResolvedValue(DENY_RESPONSE);
    const desc = sbo3lElizaKeeperHubAction({ client: { submit } });
    const out = JSON.parse(await desc.handler({}, APRP_MESSAGE));

    expect(out.decision).toBe("deny");
    expect(out.kh_execution_ref).toBeNull();
    expect(out.deny_code).toBe("policy.amount_over_limit");
    // Advisory workflow id is still surfaced so the agent / audit log
    // knows which workflow was *intended* even though execution didn't happen.
    expect(out.kh_workflow_id_advisory).toBe(DEFAULT_KH_WORKFLOW_ID);
  });

  it("honors workflowId override", async () => {
    const submit = vi.fn().mockResolvedValue(ALLOW_RESPONSE);
    const desc = sbo3lElizaKeeperHubAction({
      client: { submit },
      workflowId: "kh-staging-workflow-xyz",
    });
    const out = JSON.parse(await desc.handler({}, APRP_MESSAGE));

    expect(out.kh_workflow_id_advisory).toBe("kh-staging-workflow-xyz");
  });

  it("returns no-aprp error envelope when message.content.text is invalid JSON", async () => {
    const submit = vi.fn();
    const desc = sbo3lElizaKeeperHubAction({ client: { submit } });
    const out = JSON.parse(
      await desc.handler({}, { content: { text: "{not valid json" } }),
    );

    expect(out.error).toBe("input.no_aprp_in_message");
    expect(submit).not.toHaveBeenCalled();
  });

  it("returns no-aprp error envelope when message.content.text decodes to an array", async () => {
    const submit = vi.fn();
    const desc = sbo3lElizaKeeperHubAction({ client: { submit } });
    const out = JSON.parse(
      await desc.handler({}, { content: { text: "[1,2,3]" } }),
    );

    expect(out.error).toBe("input.no_aprp_in_message");
    expect(submit).not.toHaveBeenCalled();
  });

  it("returns structured error envelope on transport failure with code", async () => {
    const err: { code: string; status: number; message: string } = {
      code: "auth.required",
      status: 401,
      message: "missing bearer",
    };
    const submit = vi.fn().mockRejectedValue(err);
    const desc = sbo3lElizaKeeperHubAction({ client: { submit } });
    const out = JSON.parse(await desc.handler({}, APRP_MESSAGE));

    expect(out.error).toBe("auth.required");
    expect(out.status).toBe(401);
  });

  it("falls back to transport.failed on opaque exception", async () => {
    const submit = vi.fn().mockRejectedValue(new Error("network down"));
    const desc = sbo3lElizaKeeperHubAction({ client: { submit } });
    const out = JSON.parse(await desc.handler({}, APRP_MESSAGE));

    expect(out.error).toBe("transport.failed");
    expect(out.detail).toContain("network down");
  });

  it("calls idempotencyKey callback when provided", async () => {
    const submit = vi.fn().mockResolvedValue(ALLOW_RESPONSE);
    const idempotencyKey = vi.fn().mockReturnValue("idem-key-xyz");
    const desc = sbo3lElizaKeeperHubAction({
      client: { submit },
      idempotencyKey,
    });
    await desc.handler({}, APRP_MESSAGE);

    expect(idempotencyKey).toHaveBeenCalledOnce();
    expect(submit).toHaveBeenCalledWith(expect.any(Object), {
      idempotencyKey: "idem-key-xyz",
    });
  });
});
