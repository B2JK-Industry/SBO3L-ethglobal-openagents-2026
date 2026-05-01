import { describe, expect, it, vi } from "vitest";
import {
  sbo3lFunction,
  type SBO3LClientLike,
  type SBO3LSubmitResult,
} from "../src/index.js";

const APRP = {
  agent_id: "research-agent-01",
  task_id: "demo-1",
  intent: "purchase_api_call",
} as Record<string, unknown>;

const ALLOW: SBO3LSubmitResult = {
  decision: "allow",
  deny_code: null,
  matched_rule_id: "allow-low-risk-x402",
  request_hash: "c0bd2fab".repeat(8),
  policy_hash: "e044f13c".repeat(8),
  audit_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGR",
  receipt: { execution_ref: "kh-01HTAWX5K3R8YV9NQB7C6P2DGS" },
};

const DENY: SBO3LSubmitResult = {
  decision: "deny",
  deny_code: "policy.budget_exceeded",
  matched_rule_id: "daily-budget",
  request_hash: "c0bd2fab".repeat(8),
  policy_hash: "e044f13c".repeat(8),
  audit_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGT",
  receipt: { execution_ref: null },
};

function fakeClient(r: SBO3LSubmitResult): SBO3LClientLike {
  return { submit: vi.fn(async () => r) };
}

describe("sbo3lFunction — descriptor", () => {
  it("exposes name + description + parameters + call", () => {
    const f = sbo3lFunction({ client: fakeClient(ALLOW) });
    expect(f.name).toBe("sbo3l_payment_request");
    expect(typeof f.description).toBe("string");
    expect(f.parameters.type).toBe("object");
    expect(typeof f.call).toBe("function");
  });

  it("APRP parameters declares required fields", () => {
    const f = sbo3lFunction({ client: fakeClient(ALLOW) });
    const required = (f.parameters.required as string[]) ?? [];
    expect(required).toContain("agent_id");
    expect(required).toContain("intent");
    expect(required).toContain("risk_class");
  });

  it("custom name + description", () => {
    const f = sbo3lFunction({ client: fakeClient(ALLOW), name: "fn", description: "d" });
    expect(f.name).toBe("fn");
    expect(f.description).toBe("d");
  });
});

describe("sbo3lFunction — call", () => {
  it("returns flat allow envelope", async () => {
    const f = sbo3lFunction({ client: fakeClient(ALLOW) });
    const r = await f.call(APRP);
    expect(r.decision).toBe("allow");
    expect(r.execution_ref).toBe("kh-01HTAWX5K3R8YV9NQB7C6P2DGS");
  });

  it("returns deny with deny_code", async () => {
    const f = sbo3lFunction({ client: fakeClient(DENY) });
    const r = await f.call(APRP);
    expect(r.decision).toBe("deny");
    expect(r.deny_code).toBe("policy.budget_exceeded");
    expect(r.execution_ref).toBeNull();
  });

  it("forwards args to client", async () => {
    const submit = vi.fn(async () => ALLOW);
    const f = sbo3lFunction({ client: { submit } });
    await f.call(APRP);
    const [body] = submit.mock.calls[0]!;
    expect((body as { agent_id: string }).agent_id).toBe("research-agent-01");
  });

  it("invokes idempotencyKey callback", async () => {
    const submit = vi.fn(async () => ALLOW);
    const f = sbo3lFunction({
      client: { submit },
      idempotencyKey: (args) => `${(args as { task_id: string }).task_id}-key`,
    });
    await f.call(APRP);
    const [, opts] = submit.mock.calls[0]!;
    expect((opts as { idempotencyKey?: string })?.idempotencyKey).toBe("demo-1-key");
  });
});

describe("sbo3lFunction — errors", () => {
  it("surfaces SBO3L code", async () => {
    const err = Object.assign(new Error("auth"), { code: "auth.required", status: 401 });
    const client: SBO3LClientLike = { submit: vi.fn(async () => Promise.reject(err)) };
    const f = sbo3lFunction({ client });
    const r = await f.call(APRP);
    expect(r.error).toBe("auth.required");
    expect(r.status).toBe(401);
    expect(r.decision).toBeUndefined();
  });

  it("falls back to transport.failed", async () => {
    const client: SBO3LClientLike = {
      submit: vi.fn(async () => Promise.reject(new Error("ECONNREFUSED"))),
    };
    const f = sbo3lFunction({ client });
    const r = await f.call(APRP);
    expect(r.error).toBe("transport.failed");
    expect(r.detail).toContain("ECONNREFUSED");
  });
});
