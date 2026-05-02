import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SBO3LClient } from "@sbo3l/sdk";

import {
  APRP_SCHEMA,
  PolicyDenyError,
  sbo3lCommand,
  runSbo3lCommand,
  type PaymentRequest,
  type AutoGPTToolCall,
} from "../src/index.js";

const ENDPOINT = "http://sbo3l-test.local:8730";

const aprpFixture: PaymentRequest = {
  agent_id: "research-agent-01",
  task_id: "demo-autogpt-1",
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

const allow = (auditId = "evt-allow-1") => ({
  decision: "allow",
  deny_code: null,
  matched_rule_id: "allow-x402",
  request_hash: "00".repeat(32),
  policy_hash: "00".repeat(32),
  audit_event_id: auditId,
  receipt: {
    execution_ref: "kh-1",
    agent_id: aprpFixture.agent_id,
    task_id: aprpFixture.task_id,
    decision: "allow",
    audit_event_id: auditId,
    request_hash: "00".repeat(32),
    policy_hash: "00".repeat(32),
    signature: { alg: "ed25519", key_id: "k1", value: "00".repeat(32) },
    signed_at: "2099-01-01T00:00:00Z",
  },
});

const deny = (code = "policy.budget_exceeded", auditId = "evt-deny-1") => ({
  decision: "deny",
  deny_code: code,
  matched_rule_id: null,
  request_hash: "00".repeat(32),
  policy_hash: "00".repeat(32),
  audit_event_id: auditId,
  receipt: {
    execution_ref: null,
    agent_id: aprpFixture.agent_id,
    task_id: aprpFixture.task_id,
    decision: "deny",
    audit_event_id: auditId,
    request_hash: "00".repeat(32),
    policy_hash: "00".repeat(32),
    signature: { alg: "ed25519", key_id: "k1", value: "00".repeat(32) },
    signed_at: "2099-01-01T00:00:00Z",
  },
});

beforeEach(() => {
  vi.restoreAllMocks();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("APRP_SCHEMA", () => {
  it("declares 12 required APRP fields", () => {
    expect(APRP_SCHEMA.required).toHaveLength(12);
  });

  it("currency pinned to USD", () => {
    expect(APRP_SCHEMA.properties.amount.properties.currency.enum).toEqual(["USD"]);
  });
});

describe("sbo3lCommand", () => {
  it("default tool name", () => {
    const t = sbo3lCommand({ client: new SBO3LClient({ endpoint: ENDPOINT }) });
    expect(t.name).toBe("sbo3l_payment_request");
    expect(t.descriptor.name).toBe(t.name);
  });

  it("name + description overrides flow into descriptor", () => {
    const t = sbo3lCommand({
      client: new SBO3LClient({ endpoint: ENDPOINT }),
      name: "pay",
      description: "custom",
    });
    expect(t.descriptor.name).toBe("pay");
    expect(t.descriptor.description).toBe("custom");
  });

  it("execute returns receipt on allow", async () => {
    fetchOnce(allow("evt-1"));
    const t = sbo3lCommand({ client: new SBO3LClient({ endpoint: ENDPOINT }) });
    const receipt = await t.execute(aprpFixture);
    expect(receipt.execution_ref).toBe("kh-1");
  });

  it("execute throws PolicyDenyError on deny", async () => {
    fetchOnce(deny("policy.budget_exceeded", "evt-deny-2"));
    const t = sbo3lCommand({ client: new SBO3LClient({ endpoint: ENDPOINT }) });
    await expect(t.execute(aprpFixture)).rejects.toBeInstanceOf(PolicyDenyError);
  });
});

describe("runSbo3lCommand", () => {
  function call(name: string, input: object | string, id = "tc-001"): AutoGPTToolCall {
    return { id, name, input: typeof input === "string" ? input : JSON.stringify(input) };
  }

  it("forwards allow into ok=true envelope", async () => {
    fetchOnce(allow("evt-1"));
    const t = sbo3lCommand({ client: new SBO3LClient({ endpoint: ENDPOINT }) });
    const out = await runSbo3lCommand(t, call(t.name, aprpFixture));
    expect(out.ok).toBe(true);
    expect(out.tool_call_id).toBe("tc-001");
    const parsed = JSON.parse(out.output);
    expect(parsed.execution_ref).toBe("kh-1");
  });

  it("forwards deny into ok=false envelope (no throw)", async () => {
    fetchOnce(deny("policy.budget_exceeded", "evt-deny-2"));
    const t = sbo3lCommand({ client: new SBO3LClient({ endpoint: ENDPOINT }) });
    const out = await runSbo3lCommand(t, call(t.name, aprpFixture));
    expect(out.ok).toBe(false);
    const parsed = JSON.parse(out.output);
    expect(parsed.error).toBe("policy.deny");
    expect(parsed.audit_event_id).toBe("evt-deny-2");
  });

  it("rejects malformed JSON input", async () => {
    const t = sbo3lCommand({ client: new SBO3LClient({ endpoint: ENDPOINT }) });
    const out = await runSbo3lCommand(t, call(t.name, "{not-json"));
    expect(out.ok).toBe(false);
    expect(JSON.parse(out.output).error).toBe("input.bad_arguments");
  });

  it("rejects mismatched tool name", async () => {
    const t = sbo3lCommand({ client: new SBO3LClient({ endpoint: ENDPOINT }), name: "pay" });
    const out = await runSbo3lCommand(t, call("not_pay", aprpFixture));
    expect(out.ok).toBe(false);
    expect(JSON.parse(out.output).error).toBe("input.unknown_tool");
  });

  it("transport failure → ok=false envelope", async () => {
    globalThis.fetch = vi.fn().mockRejectedValue(new TypeError("network down"));
    const t = sbo3lCommand({ client: new SBO3LClient({ endpoint: ENDPOINT }) });
    const out = await runSbo3lCommand(t, call(t.name, aprpFixture));
    expect(out.ok).toBe(false);
    expect(["transport.failed", "transport.unknown"]).toContain(JSON.parse(out.output).error);
  });
});
