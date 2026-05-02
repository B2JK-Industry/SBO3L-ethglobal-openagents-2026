import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SBO3LClient } from "@sbo3l/sdk";

import {
  APRP_INPUT_SCHEMA,
  DEFAULT_TOOL_NAME,
  PolicyDenyError,
  aprpSchema,
  runSbo3lToolUse,
  sbo3lTool,
  type AnthropicToolUseBlock,
  type PaymentRequest,
} from "../src/index.js";

const ENDPOINT = "http://sbo3l-test.local:8730";

const aprpFixture: PaymentRequest = {
  agent_id: "research-agent-01",
  task_id: "demo-anthropic-1",
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

describe("APRP_INPUT_SCHEMA", () => {
  it("declares all 12 APRP top-level fields", () => {
    expect(APRP_INPUT_SCHEMA.required).toEqual([
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
    expect(APRP_INPUT_SCHEMA.properties.amount.properties.currency.enum).toEqual(["USD"]);
  });

  it("destination.type lists the four canonical destinations", () => {
    expect(APRP_INPUT_SCHEMA.properties.destination.properties.type.enum).toEqual([
      "x402_endpoint",
      "eoa",
      "smart_account",
      "erc20_transfer",
    ]);
  });
});

describe("aprpSchema (zod)", () => {
  it("accepts the canonical fixture", () => {
    expect(() => aprpSchema.parse(aprpFixture)).not.toThrow();
  });

  it("rejects unknown intent", () => {
    expect(() =>
      aprpSchema.parse({ ...aprpFixture, intent: "buy_compute_job" }),
    ).toThrow();
  });

  it("rejects amount.currency != USD", () => {
    expect(() =>
      aprpSchema.parse({
        ...aprpFixture,
        amount: { value: "0.05", currency: "EUR" },
      }),
    ).toThrow();
  });

  it("rejects malformed agent_id", () => {
    expect(() =>
      aprpSchema.parse({ ...aprpFixture, agent_id: "Has-Capital-Letters" }),
    ).toThrow();
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

  it("execute zod-validates before submitting", async () => {
    const client = new SBO3LClient({ endpoint: ENDPOINT });
    const t = sbo3lTool({ client });
    // No fetch mock — if zod validation fails, we should never reach fetch.
    await expect(
      t.execute({ ...aprpFixture, intent: "wat" }),
    ).rejects.toThrow();
  });
});

describe("runSbo3lToolUse", () => {
  function block(name: string, input: unknown, id = "tu-001"): AnthropicToolUseBlock {
    return { type: "tool_use", id, name, input };
  }

  it("forwards an allow into a JSON receipt content", async () => {
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
    const out = await runSbo3lToolUse(tool, block(tool.name, aprpFixture));
    expect(out.type).toBe("tool_result");
    expect(out.tool_use_id).toBe("tu-001");
    expect(out.is_error).toBeUndefined();
    const parsed = JSON.parse(out.content);
    expect(parsed.execution_ref).toBe("kh-1");
  });

  it("converts a deny into is_error envelope (does NOT throw)", async () => {
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
    const out = await runSbo3lToolUse(tool, block(tool.name, aprpFixture));
    expect(out.is_error).toBe(true);
    const parsed = JSON.parse(out.content);
    expect(parsed.error).toBe("policy.deny");
    expect(parsed.deny_code).toBe("policy.deny_recipient_not_allowlisted");
    expect(parsed.audit_event_id).toBe("evt-deny-2");
  });

  it("converts zod validation failure into is_error envelope with issue paths", async () => {
    const client = new SBO3LClient({ endpoint: ENDPOINT });
    const tool = sbo3lTool({ client });
    const out = await runSbo3lToolUse(
      tool,
      block(tool.name, { ...aprpFixture, intent: "not_a_valid_intent" }),
    );
    expect(out.is_error).toBe(true);
    const parsed = JSON.parse(out.content);
    expect(parsed.error).toBe("input.bad_arguments");
    expect(Array.isArray(parsed.issues)).toBe(true);
    expect(parsed.issues.length).toBeGreaterThan(0);
  });

  it("rejects a tool_use routed to a different tool name", async () => {
    const client = new SBO3LClient({ endpoint: ENDPOINT });
    const tool = sbo3lTool({ client, name: "pay" });
    const out = await runSbo3lToolUse(tool, block("not_pay", aprpFixture));
    expect(out.is_error).toBe(true);
    const parsed = JSON.parse(out.content);
    expect(parsed.error).toBe("input.unknown_tool");
  });

  it("converts a transport failure into transport.failed envelope", async () => {
    globalThis.fetch = vi.fn().mockRejectedValue(new TypeError("network down"));
    const client = new SBO3LClient({ endpoint: ENDPOINT });
    const tool = sbo3lTool({ client });
    const out = await runSbo3lToolUse(tool, block(tool.name, aprpFixture));
    expect(out.is_error).toBe(true);
    const parsed = JSON.parse(out.content);
    expect(["transport.failed", "transport.unknown"]).toContain(parsed.error);
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
    const tool = sbo3lTool({
      client,
      idempotencyKey: (a) => `key-${a.task_id}`,
    });
    await runSbo3lToolUse(tool, block(tool.name, aprpFixture));
    expect(globalThis.fetch).toHaveBeenCalledTimes(1);
  });
});
