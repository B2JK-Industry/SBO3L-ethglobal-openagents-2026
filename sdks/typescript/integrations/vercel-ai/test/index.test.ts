/**
 * @sbo3l/vercel-ai tests.
 *
 * Per QA rule (Daniel 2026-05-01): real `@sbo3l/sdk` integration — no SDK mocks.
 * We mock the underlying `fetch` (the SDK natively supports `fetch:` injection)
 * so the daemon doesn't need to be running, but the SDK code path is exercised
 * end-to-end.
 */

import { describe, expect, it, vi } from "vitest";
import { SBO3LClient, type FetchLike } from "@sbo3l/sdk";
import { sbo3lTool, PolicyDenyError, aprpSchema } from "../src/index.js";

const APRP = {
  agent_id: "research-agent-01",
  task_id: "demo-1",
  intent: "purchase_api_call" as const,
  amount: { value: "0.05", currency: "USD" as const },
  token: "USDC",
  destination: {
    type: "x402_endpoint" as const,
    url: "https://api.example.com/v1/inference",
    method: "POST" as const,
  },
  payment_protocol: "x402" as const,
  chain: "base",
  provider_url: "https://api.example.com",
  expiry: "2026-05-01T10:31:00Z",
  nonce: "01HTAWX5K3R8YV9NQB7C6P2DGM",
  risk_class: "low" as const,
};

const ALLOW_ENVELOPE = {
  status: "auto_approved",
  decision: "allow",
  deny_code: null,
  matched_rule_id: "allow-low-risk-x402",
  request_hash: "c0bd2fab".repeat(8),
  policy_hash: "e044f13c".repeat(8),
  audit_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGR",
  receipt: {
    receipt_type: "sbo3l.policy_receipt.v1",
    version: 1,
    agent_id: "research-agent-01",
    decision: "allow",
    deny_code: null,
    request_hash: "c0bd2fab".repeat(8),
    policy_hash: "e044f13c".repeat(8),
    policy_version: 1,
    audit_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGR",
    execution_ref: "kh-01HTAWX5K3R8YV9NQB7C6P2DGS",
    issued_at: "2026-04-29T10:00:00Z",
    expires_at: null,
    signature: {
      algorithm: "ed25519",
      key_id: "decision-mock-v1",
      signature_hex: "1".repeat(128),
    },
  },
};

const DENY_ENVELOPE = {
  ...ALLOW_ENVELOPE,
  status: "rejected",
  decision: "deny",
  deny_code: "policy.budget_exceeded",
  matched_rule_id: "daily-budget",
  audit_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGT",
  receipt: { ...ALLOW_ENVELOPE.receipt, decision: "deny", deny_code: "policy.budget_exceeded" },
};

function clientWithEnvelope(envelope: unknown, status = 200): SBO3LClient {
  const fakeFetch: FetchLike = async () =>
    new Response(JSON.stringify(envelope), {
      status,
      headers: { "Content-Type": "application/json" },
    });
  return new SBO3LClient({ endpoint: "http://localhost:8730", fetch: fakeFetch });
}

describe("sbo3lTool — descriptor shape", () => {
  it("returns an ai.tool() descriptor", () => {
    const t = sbo3lTool({ client: clientWithEnvelope(ALLOW_ENVELOPE) });
    // ai.tool() returns an object with `description`, `parameters`, `execute`.
    expect(typeof t.description).toBe("string");
    expect(t.parameters).toBeDefined();
    expect(typeof t.execute).toBe("function");
  });

  it("custom description threads through", () => {
    const t = sbo3lTool({
      client: clientWithEnvelope(ALLOW_ENVELOPE),
      description: "custom-desc",
    });
    expect(t.description).toBe("custom-desc");
  });
});

describe("aprpSchema — zod parameter validation", () => {
  it("accepts the golden APRP", () => {
    const r = aprpSchema.safeParse(APRP);
    expect(r.success).toBe(true);
  });

  it("rejects bad agent_id", () => {
    const bad = { ...APRP, agent_id: "INVALID-UPPERCASE" };
    const r = aprpSchema.safeParse(bad);
    expect(r.success).toBe(false);
  });

  it("rejects non-USD currency", () => {
    const bad = { ...APRP, amount: { value: "0.05", currency: "EUR" as unknown as "USD" } };
    const r = aprpSchema.safeParse(bad);
    expect(r.success).toBe(false);
  });

  it("rejects unknown intent", () => {
    const bad = { ...APRP, intent: "purchase_unknown" as unknown as typeof APRP.intent };
    const r = aprpSchema.safeParse(bad);
    expect(r.success).toBe(false);
  });

  it("rejects malformed nonce (non-ULID)", () => {
    const bad = { ...APRP, nonce: "not-a-ulid" };
    const r = aprpSchema.safeParse(bad);
    expect(r.success).toBe(false);
  });
});

describe("sbo3lTool.execute — allow path", () => {
  it("returns the PolicyReceipt on allow", async () => {
    const t = sbo3lTool({ client: clientWithEnvelope(ALLOW_ENVELOPE) });
    const r = await t.execute!(APRP, {});
    expect(r.decision).toBe("allow");
    expect(r.execution_ref).toBe("kh-01HTAWX5K3R8YV9NQB7C6P2DGS");
    expect(r.signature.algorithm).toBe("ed25519");
  });

  it("forwards APRP body to SDK", async () => {
    const captured: Array<unknown> = [];
    const fakeFetch: FetchLike = async (_url, init) => {
      if (init?.body !== undefined) captured.push(JSON.parse(init.body as string));
      return new Response(JSON.stringify(ALLOW_ENVELOPE), { status: 200 });
    };
    const client = new SBO3LClient({ endpoint: "http://localhost:8730", fetch: fakeFetch });
    const t = sbo3lTool({ client });
    await t.execute!(APRP, {});
    expect(captured).toHaveLength(1);
    expect((captured[0] as { agent_id: string }).agent_id).toBe("research-agent-01");
  });

  it("invokes idempotencyKey callback", async () => {
    const captured: Array<Record<string, string>> = [];
    const fakeFetch: FetchLike = async (_url, init) => {
      const headers = init?.headers as Record<string, string> | undefined;
      if (headers !== undefined) captured.push(headers);
      return new Response(JSON.stringify(ALLOW_ENVELOPE), { status: 200 });
    };
    const client = new SBO3LClient({ endpoint: "http://localhost:8730", fetch: fakeFetch });
    const t = sbo3lTool({
      client,
      idempotencyKey: (a) => `${a.task_id}-key`,
    });
    await t.execute!(APRP, {});
    expect(captured[0]?.["Idempotency-Key"]).toBe("demo-1-key");
  });
});

describe("sbo3lTool.execute — deny path throws PolicyDenyError", () => {
  it("throws PolicyDenyError on deny", async () => {
    const t = sbo3lTool({ client: clientWithEnvelope(DENY_ENVELOPE) });
    await expect(t.execute!(APRP, {})).rejects.toBeInstanceOf(
      PolicyDenyError,
    );
  });

  it("PolicyDenyError carries deny_code and decision", async () => {
    const t = sbo3lTool({ client: clientWithEnvelope(DENY_ENVELOPE) });
    try {
      await t.execute!(APRP, {});
      expect.fail("expected throw");
    } catch (err) {
      expect(err).toBeInstanceOf(PolicyDenyError);
      const e = err as PolicyDenyError;
      expect(e.decision).toBe("deny");
      expect(e.denyCode).toBe("policy.budget_exceeded");
      expect(e.matchedRuleId).toBe("daily-budget");
      expect(e.auditEventId).toBe("evt-01HTAWX5K3R8YV9NQB7C6P2DGT");
    }
  });

  it("PolicyDenyError message includes deny code", async () => {
    const t = sbo3lTool({ client: clientWithEnvelope(DENY_ENVELOPE) });
    try {
      await t.execute!(APRP, {});
      expect.fail("expected throw");
    } catch (err) {
      expect((err as Error).message).toContain("policy.budget_exceeded");
    }
  });
});

describe("sbo3lTool.execute — transport errors bubble", () => {
  it("SBO3LError on 401 from daemon", async () => {
    const problem = {
      type: "https://schemas.sbo3l.dev/errors/auth.required",
      title: "Authentication required",
      status: 401,
      detail: "missing",
      code: "auth.required",
    };
    const t = sbo3lTool({ client: clientWithEnvelope(problem, 401) });
    await expect(t.execute!(APRP, {})).rejects.toMatchObject({
      code: "auth.required",
      status: 401,
    });
  });

  it("network failure bubbles as SBO3LTransportError", async () => {
    const fakeFetch: FetchLike = vi.fn(async () => {
      throw new TypeError("ECONNREFUSED");
    });
    const client = new SBO3LClient({ endpoint: "http://localhost:8730", fetch: fakeFetch });
    const t = sbo3lTool({ client });
    await expect(t.execute!(APRP, {})).rejects.toThrow(
      /ECONNREFUSED/,
    );
  });
});
