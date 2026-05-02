import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SBO3LClient } from "@sbo3l/sdk";

import {
  DEFAULT_TOOL_ID,
  PolicyDenyError,
  inputSchema,
  outputSchema,
  sbo3lTool,
  type PaymentRequest,
} from "../src/index.js";

const ENDPOINT = "http://sbo3l-test.local:8730";

const aprpFixture: PaymentRequest = {
  agent_id: "research-agent-01",
  task_id: "demo-mastra-1",
  intent: "purchase_api_call",
  amount: { value: "0.05", currency: "USD" },
  token: "USDC",
  destination: {
    type: "x402_endpoint",
    url: "https://api.example.com/v1/inference",
    method: "POST",
    expected_recipient: "0x1111111111111111111111111111111111111111",
  },
  payment_protocol: "x402",
  chain: "base",
  provider_url: "https://api.example.com",
  expiry: "2099-01-01T00:00:00Z",
  nonce: "01HTAWX5K3R8YV9NQB7C6P2DGM",
  risk_class: "low",
};

function fetchOnce(response: object, status = 200): void {
  globalThis.fetch = vi.fn().mockResolvedValue({
    ok: status >= 200 && status < 300,
    status,
    headers: new Headers({ "content-type": "application/json" }),
    json: async () => response,
    text: async () => JSON.stringify(response),
  });
}

beforeEach(() => {
  vi.restoreAllMocks();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("inputSchema", () => {
  it("accepts the canonical fixture", () => {
    expect(() => inputSchema.parse(aprpFixture)).not.toThrow();
  });

  it("rejects unknown intent", () => {
    expect(() =>
      inputSchema.parse({ ...aprpFixture, intent: "buy_compute_job" }),
    ).toThrow();
  });

  it("rejects amount.currency != USD", () => {
    expect(() =>
      inputSchema.parse({ ...aprpFixture, amount: { value: "0.05", currency: "EUR" } }),
    ).toThrow();
  });
});

describe("outputSchema", () => {
  it("decision is pinned to allow (deny path throws)", () => {
    expect(() =>
      outputSchema.parse({
        decision: "allow",
        audit_event_id: "evt-1",
        execution_ref: "kh-1",
        receipt: {},
      }),
    ).not.toThrow();
    expect(() =>
      outputSchema.parse({
        decision: "deny",
        audit_event_id: "evt-1",
        execution_ref: null,
        receipt: {},
      }),
    ).toThrow();
  });
});

describe("sbo3lTool", () => {
  it("default tool id is sbo3l_payment_request", () => {
    const client = new SBO3LClient({ endpoint: ENDPOINT });
    const t = sbo3lTool({ client });
    expect(t.id).toBe(DEFAULT_TOOL_ID);
  });

  it("id + description overrides flow into the descriptor", () => {
    const client = new SBO3LClient({ endpoint: ENDPOINT });
    const t = sbo3lTool({ client, id: "pay", description: "custom" });
    expect(t.id).toBe("pay");
    expect(t.description).toBe("custom");
  });

  it("execute returns Mastra-shaped output on allow", async () => {
    fetchOnce({
      decision: "allow",
      deny_code: null,
      matched_rule_id: "allow-small-x402-api-call",
      request_hash: "00".repeat(32),
      policy_hash: "00".repeat(32),
      audit_event_id: "evt-allow-1",
      receipt: {
        execution_ref: "kh-allow-1",
        agent_id: aprpFixture.agent_id,
        task_id: aprpFixture.task_id,
        decision: "allow",
        request_hash: "00".repeat(32),
        policy_hash: "00".repeat(32),
        signature: { alg: "ed25519", key_id: "k1", value: "00".repeat(32) },
        signed_at: "2099-01-01T00:00:00Z",
      },
    });
    const client = new SBO3LClient({ endpoint: ENDPOINT });
    const t = sbo3lTool({ client });
    const out = await t.execute({ context: aprpFixture });
    expect(out.decision).toBe("allow");
    expect(out.audit_event_id).toBe("evt-allow-1");
    expect(out.execution_ref).toBe("kh-allow-1");
  });

  it("execute throws PolicyDenyError on deny", async () => {
    fetchOnce({
      decision: "deny",
      deny_code: "policy.budget_exceeded",
      matched_rule_id: "cap-per-tx",
      request_hash: "00".repeat(32),
      policy_hash: "00".repeat(32),
      audit_event_id: "evt-deny-1",
      receipt: {
        execution_ref: null,
        agent_id: aprpFixture.agent_id,
        task_id: aprpFixture.task_id,
        decision: "deny",
        request_hash: "00".repeat(32),
        policy_hash: "00".repeat(32),
        signature: { alg: "ed25519", key_id: "k1", value: "00".repeat(32) },
        signed_at: "2099-01-01T00:00:00Z",
      },
    });
    const client = new SBO3LClient({ endpoint: ENDPOINT });
    const t = sbo3lTool({ client });
    await expect(t.execute({ context: aprpFixture })).rejects.toBeInstanceOf(PolicyDenyError);
  });

  it("idempotencyKey callback is forwarded", async () => {
    fetchOnce({
      decision: "allow",
      deny_code: null,
      matched_rule_id: "allow-small-x402-api-call",
      request_hash: "00".repeat(32),
      policy_hash: "00".repeat(32),
      audit_event_id: "evt-idem-1",
      receipt: {
        execution_ref: "kh-idem-1",
        agent_id: aprpFixture.agent_id,
        task_id: aprpFixture.task_id,
        decision: "allow",
        request_hash: "00".repeat(32),
        policy_hash: "00".repeat(32),
        signature: { alg: "ed25519", key_id: "k1", value: "00".repeat(32) },
        signed_at: "2099-01-01T00:00:00Z",
      },
    });
    const client = new SBO3LClient({ endpoint: ENDPOINT });
    const t = sbo3lTool({ client, idempotencyKey: (a) => `key-${a.task_id}` });
    await t.execute({ context: aprpFixture });
    expect(globalThis.fetch).toHaveBeenCalledTimes(1);
  });
});
