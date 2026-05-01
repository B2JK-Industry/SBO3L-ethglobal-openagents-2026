import { describe, expect, it, vi } from "vitest";
import { sbo3lTool, type SBO3LClientLike, type SBO3LSubmitResult } from "../src/index.js";

const APRP = JSON.stringify({
  agent_id: "research-agent-01",
  task_id: "demo-1",
  intent: "purchase_api_call",
});

const ALLOW_RESULT: SBO3LSubmitResult = {
  decision: "allow",
  deny_code: null,
  matched_rule_id: "allow-low-risk-x402",
  request_hash: "c0bd2fab".repeat(8),
  policy_hash: "e044f13c".repeat(8),
  audit_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGR",
  receipt: { execution_ref: "kh-01HTAWX5K3R8YV9NQB7C6P2DGS" },
};

const DENY_RESULT: SBO3LSubmitResult = {
  decision: "deny",
  deny_code: "policy.budget_exceeded",
  matched_rule_id: "daily-budget",
  request_hash: "c0bd2fab".repeat(8),
  policy_hash: "e044f13c".repeat(8),
  audit_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGT",
  receipt: { execution_ref: null },
};

function fakeClient(result: SBO3LSubmitResult): SBO3LClientLike {
  return { submit: vi.fn(async () => result) };
}

describe("sbo3lTool — descriptor shape", () => {
  it("returns name + description + func", () => {
    const t = sbo3lTool({ client: fakeClient(ALLOW_RESULT) });
    expect(t.name).toBe("sbo3l_payment_request");
    expect(t.description.length).toBeGreaterThan(50);
    expect(typeof t.func).toBe("function");
  });

  it("accepts custom name and description", () => {
    const t = sbo3lTool({
      client: fakeClient(ALLOW_RESULT),
      name: "my_tool",
      description: "custom",
    });
    expect(t.name).toBe("my_tool");
    expect(t.description).toBe("custom");
  });
});

describe("sbo3lTool — happy path", () => {
  it("returns allow envelope as JSON string", async () => {
    const t = sbo3lTool({ client: fakeClient(ALLOW_RESULT) });
    const out = JSON.parse(await t.func(APRP));
    expect(out.decision).toBe("allow");
    expect(out.execution_ref).toBe("kh-01HTAWX5K3R8YV9NQB7C6P2DGS");
    expect(out.matched_rule_id).toBe("allow-low-risk-x402");
  });

  it("returns deny envelope with deny_code", async () => {
    const t = sbo3lTool({ client: fakeClient(DENY_RESULT) });
    const out = JSON.parse(await t.func(APRP));
    expect(out.decision).toBe("deny");
    expect(out.deny_code).toBe("policy.budget_exceeded");
    expect(out.execution_ref).toBeNull();
  });

  it("forwards APRP body to client", async () => {
    const submit = vi.fn(async () => ALLOW_RESULT);
    const t = sbo3lTool({ client: { submit } });
    await t.func(APRP);
    expect(submit).toHaveBeenCalledOnce();
    const [body] = submit.mock.calls[0]!;
    expect((body as { agent_id: string }).agent_id).toBe("research-agent-01");
  });
});

describe("sbo3lTool — input validation", () => {
  it("rejects non-JSON input", async () => {
    const t = sbo3lTool({ client: fakeClient(ALLOW_RESULT) });
    const out = JSON.parse(await t.func("not-json"));
    expect(out.error).toBe("input is not valid JSON");
  });

  it("rejects array input", async () => {
    const t = sbo3lTool({ client: fakeClient(ALLOW_RESULT) });
    const out = JSON.parse(await t.func("[1,2,3]"));
    expect(out.error).toBe("input must be a JSON object (APRP)");
    expect(out.input_received_type).toBe("array");
  });

  it("rejects null input", async () => {
    const t = sbo3lTool({ client: fakeClient(ALLOW_RESULT) });
    const out = JSON.parse(await t.func("null"));
    expect(out.error).toBe("input must be a JSON object (APRP)");
    expect(out.input_received_type).toBe("null");
  });
});

describe("sbo3lTool — error handling", () => {
  it("surfaces SBO3LError code", async () => {
    const err = Object.assign(new Error("auth required"), {
      code: "auth.required",
      status: 401,
    });
    const client: SBO3LClientLike = { submit: vi.fn(async () => Promise.reject(err)) };
    const t = sbo3lTool({ client });
    const out = JSON.parse(await t.func(APRP));
    expect(out.error).toBe("auth.required");
    expect(out.status).toBe(401);
  });

  it("falls back to transport.failed on plain Error", async () => {
    const client: SBO3LClientLike = {
      submit: vi.fn(async () => Promise.reject(new Error("ECONNREFUSED"))),
    };
    const t = sbo3lTool({ client });
    const out = JSON.parse(await t.func(APRP));
    expect(out.error).toBe("transport.failed");
    expect(out.detail).toContain("ECONNREFUSED");
  });
});

describe("sbo3lTool — idempotency", () => {
  it("invokes idempotencyKey callback per call", async () => {
    const submit = vi.fn(async () => ALLOW_RESULT);
    const t = sbo3lTool({
      client: { submit },
      idempotencyKey: (input) => `${(input as { task_id: string }).task_id}-key`,
    });
    await t.func(APRP);
    const [, opts] = submit.mock.calls[0]!;
    expect((opts as { idempotencyKey?: string })?.idempotencyKey).toBe("demo-1-key");
  });
});
