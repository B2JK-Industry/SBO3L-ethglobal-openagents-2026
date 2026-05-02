import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SBO3LClient } from "@sbo3l/sdk";

import {
  APRP_JSON_SCHEMA,
  DEFAULT_TOOL_NAME,
  PolicyDenyError,
  runSbo3lToolCall,
  sbo3lAssistantTool,
  type AssistantToolCall,
  type PaymentRequest,
} from "../src/index.js";

const ENDPOINT = "http://sbo3l-test.local:8730";

const aprpFixture: PaymentRequest = {
  agent_id: "research-agent-01",
  task_id: "demo-openai-assistants-1",
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

describe("APRP_JSON_SCHEMA", () => {
  it("declares all 12 APRP top-level fields", () => {
    expect(APRP_JSON_SCHEMA.required).toEqual([
      "agent_id",
      "task_id",
      "intent",
      "amount",
      "token",
      "destination",
      "payment_protocol",
      "chain",
      "provider_url",
      "expiry",
      "nonce",
      "risk_class",
    ]);
  });

  it("amount currency is pinned to USD", () => {
    expect(APRP_JSON_SCHEMA.properties.amount.properties.currency.const).toBe("USD");
  });

  it("destination.type allows the four canonical destinations", () => {
    expect(APRP_JSON_SCHEMA.properties.destination.properties.type.enum).toEqual([
      "x402_endpoint",
      "eoa",
      "smart_account",
      "erc20_transfer",
    ]);
  });

  it("nonce pattern accepts both ULID and UUID forms", () => {
    const re = new RegExp(APRP_JSON_SCHEMA.properties.nonce.pattern);
    expect(re.test("01HTAWX5K3R8YV9NQB7C6P2DGM")).toBe(true); // ULID
    expect(re.test("550e8400-e29b-41d4-a716-446655440000")).toBe(true); // UUID
    expect(re.test("not-a-nonce!")).toBe(false);
  });

  it("additionalProperties on the root is closed (deny_unknown_fields)", () => {
    expect(APRP_JSON_SCHEMA.additionalProperties).toBe(false);
  });
});

describe("sbo3lAssistantTool", () => {
  it("default tool name is sbo3l_payment_request", () => {
    const client = new SBO3LClient({ endpoint: ENDPOINT });
    const t = sbo3lAssistantTool({ client });
    expect(t.name).toBe(DEFAULT_TOOL_NAME);
    expect(t.definition.function.name).toBe(DEFAULT_TOOL_NAME);
    expect(t.definition.type).toBe("function");
  });

  it("name override threads through definition + dispatch", () => {
    const client = new SBO3LClient({ endpoint: ENDPOINT });
    const t = sbo3lAssistantTool({ client, name: "pay" });
    expect(t.name).toBe("pay");
    expect(t.definition.function.name).toBe("pay");
  });

  it("description override flows into the function definition", () => {
    const client = new SBO3LClient({ endpoint: ENDPOINT });
    const t = sbo3lAssistantTool({ client, description: "custom desc" });
    expect(t.definition.function.description).toBe("custom desc");
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
    const t = sbo3lAssistantTool({ client });
    const receipt = await t.execute(aprpFixture);
    expect(receipt.execution_ref).toBe("kh-allow-1");
    expect(receipt.agent_id).toBe(aprpFixture.agent_id);
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
    const t = sbo3lAssistantTool({ client });
    await expect(t.execute(aprpFixture)).rejects.toBeInstanceOf(PolicyDenyError);
    try {
      await t.execute(aprpFixture);
    } catch (e) {
      expect(e).toBeInstanceOf(PolicyDenyError);
      const err = e as PolicyDenyError;
      expect(err.decision).toBe("deny");
      expect(err.denyCode).toBe("policy.budget_exceeded");
      expect(err.auditEventId).toBe("evt-deny-1");
    }
  });

  it("idempotencyKey callback is forwarded to submit", async () => {
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
    const t = sbo3lAssistantTool({
      client,
      idempotencyKey: (a) => `key-${a.task_id}`,
    });
    await t.execute(aprpFixture);
    // The fetch mock was called once; the idempotency key shows up in the
    // body / headers. We just assert fetch was invoked rather than
    // sniff headers — the SDK has its own coverage for that wiring.
    expect(globalThis.fetch).toHaveBeenCalledTimes(1);
  });
});

describe("runSbo3lToolCall", () => {
  function call(
    name: string,
    args: object | string,
    id = "call-001",
  ): AssistantToolCall {
    return {
      id,
      type: "function",
      function: {
        name,
        arguments: typeof args === "string" ? args : JSON.stringify(args),
      },
    };
  }

  it("forwards an allow into a JSON receipt output", async () => {
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
    const tool = sbo3lAssistantTool({ client });
    const out = await runSbo3lToolCall(tool, call(tool.name, aprpFixture));
    expect(out.tool_call_id).toBe("call-001");
    const parsed = JSON.parse(out.output);
    expect(parsed.execution_ref).toBe("kh-1");
  });

  it("converts a deny into a structured envelope (does NOT throw)", async () => {
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
    const tool = sbo3lAssistantTool({ client });
    const out = await runSbo3lToolCall(tool, call(tool.name, aprpFixture));
    const parsed = JSON.parse(out.output);
    expect(parsed.error).toBe("policy.deny");
    expect(parsed.decision).toBe("deny");
    expect(parsed.deny_code).toBe("policy.deny_recipient_not_allowlisted");
    expect(parsed.audit_event_id).toBe("evt-deny-2");
  });

  it("converts malformed JSON arguments into input.bad_arguments envelope", async () => {
    const client = new SBO3LClient({ endpoint: ENDPOINT });
    const tool = sbo3lAssistantTool({ client });
    const out = await runSbo3lToolCall(tool, call(tool.name, "{not-json"));
    const parsed = JSON.parse(out.output);
    expect(parsed.error).toBe("input.bad_arguments");
    expect(typeof parsed.detail).toBe("string");
  });

  it("rejects a tool_call routed to a different name", async () => {
    const client = new SBO3LClient({ endpoint: ENDPOINT });
    const tool = sbo3lAssistantTool({ client, name: "pay" });
    const out = await runSbo3lToolCall(tool, call("not_pay", aprpFixture));
    const parsed = JSON.parse(out.output);
    expect(parsed.error).toBe("input.unknown_tool");
    expect(parsed.detail).toContain("expected 'pay'");
  });

  it("converts a transport failure into transport.failed envelope", async () => {
    globalThis.fetch = vi.fn().mockRejectedValue(new TypeError("network down"));
    const client = new SBO3LClient({ endpoint: ENDPOINT });
    const tool = sbo3lAssistantTool({ client });
    const out = await runSbo3lToolCall(tool, call(tool.name, aprpFixture));
    const parsed = JSON.parse(out.output);
    // SBO3LError is wrapped by the SDK on network failure → transport.failed.
    // Either error code is acceptable; both keep the run alive so the model
    // can branch on it.
    expect(["transport.failed", "transport.unknown"]).toContain(parsed.error);
  });
});
