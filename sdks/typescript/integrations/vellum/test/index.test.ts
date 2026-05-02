import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SBO3LClient } from "@sbo3l/sdk";

import {
  APRP_PARAMETERS_SCHEMA,
  DEFAULT_TOOL_NAME,
  PolicyDenyError,
  runSbo3lFunctionCall,
  sbo3lTool,
  type PaymentRequest,
  type VellumFunctionCall,
} from "../src/index.js";

const ENDPOINT = "http://sbo3l-test.local:8730";

const aprpFixture: PaymentRequest = {
  agent_id: "research-agent-01",
  task_id: "demo-vellum-1",
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

describe("APRP_PARAMETERS_SCHEMA", () => {
  it("declares 12 required APRP top-level fields", () => {
    expect(APRP_PARAMETERS_SCHEMA.required).toHaveLength(12);
    expect(APRP_PARAMETERS_SCHEMA.required).toContain("agent_id");
    expect(APRP_PARAMETERS_SCHEMA.required).toContain("nonce");
  });

  it("amount currency pinned to USD", () => {
    expect(APRP_PARAMETERS_SCHEMA.properties.amount.properties.currency.enum).toEqual(["USD"]);
  });
});

describe("sbo3lTool", () => {
  it("default tool name is sbo3l_payment_request", () => {
    const client = new SBO3LClient({ endpoint: ENDPOINT });
    const t = sbo3lTool({ client });
    expect(t.name).toBe(DEFAULT_TOOL_NAME);
    expect(t.definition.name).toBe(DEFAULT_TOOL_NAME);
  });

  it("name + description overrides flow into the definition", () => {
    const client = new SBO3LClient({ endpoint: ENDPOINT });
    const t = sbo3lTool({ client, name: "pay", description: "custom" });
    expect(t.definition.name).toBe("pay");
    expect(t.definition.description).toBe("custom");
  });

  it("execute returns the receipt on allow", async () => {
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
    const receipt = await t.execute(aprpFixture);
    expect(receipt.execution_ref).toBe("kh-allow-1");
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
    await expect(t.execute(aprpFixture)).rejects.toBeInstanceOf(PolicyDenyError);
  });
});

describe("runSbo3lFunctionCall", () => {
  function call(name: string, args: object | string): VellumFunctionCall {
    return {
      name,
      arguments: typeof args === "string" ? args : JSON.stringify(args),
    };
  }

  it("forwards an allow into a JSON receipt output (is_error=false)", async () => {
    fetchOnce({
      decision: "allow",
      deny_code: null,
      matched_rule_id: "allow-small-x402-api-call",
      request_hash: "00".repeat(32),
      policy_hash: "00".repeat(32),
      audit_event_id: "evt-1",
      receipt: {
        execution_ref: "kh-1",
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
    const tool = sbo3lTool({ client });
    const out = await runSbo3lFunctionCall(tool, call(tool.name, aprpFixture));
    expect(out.is_error).toBe(false);
    const parsed = JSON.parse(out.output);
    expect(parsed.execution_ref).toBe("kh-1");
  });

  it("converts a deny into is_error=true envelope (does NOT throw)", async () => {
    fetchOnce({
      decision: "deny",
      deny_code: "policy.deny_recipient_not_allowlisted",
      matched_rule_id: null,
      request_hash: "00".repeat(32),
      policy_hash: "00".repeat(32),
      audit_event_id: "evt-deny-2",
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
    const tool = sbo3lTool({ client });
    const out = await runSbo3lFunctionCall(tool, call(tool.name, aprpFixture));
    expect(out.is_error).toBe(true);
    const parsed = JSON.parse(out.output);
    expect(parsed.error).toBe("policy.deny");
    expect(parsed.audit_event_id).toBe("evt-deny-2");
  });

  it("converts malformed JSON arguments to input.bad_arguments envelope", async () => {
    const client = new SBO3LClient({ endpoint: ENDPOINT });
    const tool = sbo3lTool({ client });
    const out = await runSbo3lFunctionCall(tool, call(tool.name, "{not-json"));
    expect(out.is_error).toBe(true);
    const parsed = JSON.parse(out.output);
    expect(parsed.error).toBe("input.bad_arguments");
  });

  it("rejects function call routed to a different tool name", async () => {
    const client = new SBO3LClient({ endpoint: ENDPOINT });
    const tool = sbo3lTool({ client, name: "pay" });
    const out = await runSbo3lFunctionCall(tool, call("not_pay", aprpFixture));
    expect(out.is_error).toBe(true);
    const parsed = JSON.parse(out.output);
    expect(parsed.error).toBe("input.unknown_tool");
  });

  it("converts a transport failure to transport.failed envelope", async () => {
    globalThis.fetch = vi.fn().mockRejectedValue(new TypeError("network down"));
    const client = new SBO3LClient({ endpoint: ENDPOINT });
    const tool = sbo3lTool({ client });
    const out = await runSbo3lFunctionCall(tool, call(tool.name, aprpFixture));
    expect(out.is_error).toBe(true);
    const parsed = JSON.parse(out.output);
    expect(["transport.failed", "transport.unknown"]).toContain(parsed.error);
  });
});
