import { describe, expect, it, vi } from "vitest";
import {
  sbo3lPlugin,
  type SBO3LClientLike,
  type SBO3LSubmitResult,
} from "../src/index.js";

const APRP = {
  agent_id: "research-agent-01",
  task_id: "demo-1",
  intent: "purchase_api_call",
};

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

describe("sbo3lPlugin — shape", () => {
  it("exports name + description + actions", () => {
    const p = sbo3lPlugin({ client: fakeClient(ALLOW) });
    expect(p.name).toBe("@sbo3l/elizaos");
    expect(p.actions).toHaveLength(1);
    expect(p.actions[0]?.name).toBe("SBO3L_PAYMENT_REQUEST");
    expect(p.evaluators).toEqual([]);
    expect(p.providers).toEqual([]);
  });

  it("similes include PAY-like aliases", () => {
    const p = sbo3lPlugin({ client: fakeClient(ALLOW) });
    expect(p.actions[0]?.similes).toContain("PAY");
    expect(p.actions[0]?.similes).toContain("PURCHASE");
  });

  it("custom action name", () => {
    const p = sbo3lPlugin({ client: fakeClient(ALLOW), actionName: "MY_PAY" });
    expect(p.actions[0]?.name).toBe("MY_PAY");
  });
});

describe("sbo3lPlugin — validate", () => {
  it("returns true when message.content.aprp is present", async () => {
    const p = sbo3lPlugin({ client: fakeClient(ALLOW) });
    const r = await p.actions[0]!.validate({}, { content: { aprp: APRP } });
    expect(r).toBe(true);
  });

  it("returns true when message.content.text is JSON APRP", async () => {
    const p = sbo3lPlugin({ client: fakeClient(ALLOW) });
    const r = await p.actions[0]!.validate({}, { content: { text: JSON.stringify(APRP) } });
    expect(r).toBe(true);
  });

  it("returns false when no APRP can be extracted", async () => {
    const p = sbo3lPlugin({ client: fakeClient(ALLOW) });
    const r = await p.actions[0]!.validate({}, { content: { text: "hello" } });
    expect(r).toBe(false);
  });
});

describe("sbo3lPlugin — handler happy path", () => {
  it("returns allow envelope (object aprp)", async () => {
    const p = sbo3lPlugin({ client: fakeClient(ALLOW) });
    const out = await p.actions[0]!.handler(
      {},
      { content: { aprp: APRP } },
    );
    const env = JSON.parse(out);
    expect(env.decision).toBe("allow");
    expect(env.execution_ref).toBe("kh-01HTAWX5K3R8YV9NQB7C6P2DGS");
  });

  it("returns deny envelope (text-encoded aprp)", async () => {
    const p = sbo3lPlugin({ client: fakeClient(DENY) });
    const out = await p.actions[0]!.handler(
      {},
      { content: { text: JSON.stringify(APRP) } },
    );
    const env = JSON.parse(out);
    expect(env.decision).toBe("deny");
    expect(env.deny_code).toBe("policy.budget_exceeded");
  });

  it("invokes callback with the envelope text", async () => {
    const p = sbo3lPlugin({ client: fakeClient(ALLOW) });
    const callback = vi.fn();
    await p.actions[0]!.handler(
      {},
      { content: { aprp: APRP } },
      undefined,
      undefined,
      callback,
    );
    expect(callback).toHaveBeenCalledOnce();
    const env = JSON.parse(callback.mock.calls[0]![0].text);
    expect(env.decision).toBe("allow");
  });
});

describe("sbo3lPlugin — handler error paths", () => {
  it("emits no-aprp error when extraction fails", async () => {
    const p = sbo3lPlugin({ client: fakeClient(ALLOW) });
    const out = await p.actions[0]!.handler({}, { content: { text: "no APRP here" } });
    const env = JSON.parse(out);
    expect(env.error).toBe("input.no_aprp_in_message");
  });

  it("surfaces SBO3LError code", async () => {
    const err = Object.assign(new Error("auth"), { code: "auth.required", status: 401 });
    const client: SBO3LClientLike = { submit: vi.fn(async () => Promise.reject(err)) };
    const p = sbo3lPlugin({ client });
    const out = await p.actions[0]!.handler({}, { content: { aprp: APRP } });
    const env = JSON.parse(out);
    expect(env.error).toBe("auth.required");
    expect(env.status).toBe(401);
  });

  it("falls back to transport.failed", async () => {
    const client: SBO3LClientLike = {
      submit: vi.fn(async () => Promise.reject(new Error("ECONNREFUSED"))),
    };
    const p = sbo3lPlugin({ client });
    const out = await p.actions[0]!.handler({}, { content: { aprp: APRP } });
    const env = JSON.parse(out);
    expect(env.error).toBe("transport.failed");
  });
});

describe("sbo3lPlugin — idempotency + custom extractor", () => {
  it("invokes idempotencyKey callback", async () => {
    const submit = vi.fn(async () => ALLOW);
    const p = sbo3lPlugin({
      client: { submit },
      idempotencyKey: (a) => `${(a as { task_id: string }).task_id}-key`,
    });
    await p.actions[0]!.handler({}, { content: { aprp: APRP } });
    const [, opts] = submit.mock.calls[0]!;
    expect((opts as { idempotencyKey?: string })?.idempotencyKey).toBe("demo-1-key");
  });

  it("custom extractAprp wins over default", async () => {
    const submit = vi.fn(async () => ALLOW);
    const p = sbo3lPlugin({
      client: { submit },
      extractAprp: () => ({ agent_id: "custom-extracted", task_id: "x" }),
    });
    await p.actions[0]!.handler({}, { content: { text: "ignored" } });
    const [body] = submit.mock.calls[0]!;
    expect((body as { agent_id: string }).agent_id).toBe("custom-extracted");
  });
});
