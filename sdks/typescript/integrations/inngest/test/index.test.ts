import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SBO3LClient } from "@sbo3l/sdk";

import {
  PolicyDenyError,
  gateAprp,
  gateAprpSafe,
  type InngestStepLike,
  type PaymentRequest,
} from "../src/index.js";

const ENDPOINT = "http://sbo3l-test.local:8730";

const aprpFixture: PaymentRequest = {
  agent_id: "research-agent-01",
  task_id: "demo-inngest-1",
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

/**
 * Mock Inngest step.run — captures the step id + executes the handler
 * exactly once. Provides a `replay()` method that returns the cached
 * result without re-running the handler, modeling Inngest's persistence.
 */
function makeStep(): InngestStepLike & { calls: Array<{ id: string }>; cache: Map<string, unknown>; replay: <T>(id: string) => T | undefined } {
  const calls: Array<{ id: string }> = [];
  const cache = new Map<string, unknown>();
  return {
    calls,
    cache,
    async run<T>(id: string, handler: () => Promise<T>): Promise<T> {
      calls.push({ id });
      if (cache.has(id)) return cache.get(id) as T;
      const out = await handler();
      cache.set(id, out);
      return out;
    },
    replay<T>(id: string): T | undefined {
      return cache.get(id) as T | undefined;
    },
  };
}

beforeEach(() => {
  vi.restoreAllMocks();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("gateAprp (durable wrapper)", () => {
  it("step.run is invoked with stable id derived from task_id", async () => {
    fetchOnce(allow());
    const step = makeStep();
    const sbo3l = new SBO3LClient({ endpoint: ENDPOINT });
    await gateAprp(step, sbo3l, aprpFixture);
    expect(step.calls).toHaveLength(1);
    expect(step.calls[0]?.id).toBe(`sbo3l.submit:${aprpFixture.task_id}`);
  });

  it("returns the receipt on allow", async () => {
    fetchOnce(allow("evt-allow-2"));
    const step = makeStep();
    const sbo3l = new SBO3LClient({ endpoint: ENDPOINT });
    const receipt = await gateAprp(step, sbo3l, aprpFixture);
    expect(receipt.execution_ref).toBe("kh-1");
    expect(receipt.audit_event_id).toBe("evt-allow-2");
  });

  it("throws PolicyDenyError on deny (so caller can wrap NonRetriableError)", async () => {
    fetchOnce(deny("policy.budget_exceeded", "evt-deny-2"));
    const step = makeStep();
    const sbo3l = new SBO3LClient({ endpoint: ENDPOINT });
    await expect(gateAprp(step, sbo3l, aprpFixture)).rejects.toBeInstanceOf(PolicyDenyError);
    try {
      await gateAprp(step, sbo3l, aprpFixture);
    } catch (e) {
      if (e instanceof PolicyDenyError) {
        expect(e.decision).toBe("deny");
        expect(e.denyCode).toBe("policy.budget_exceeded");
      }
    }
  });

  it("on retry (cached step), submit is NOT re-called — replay path", async () => {
    fetchOnce(allow("evt-original"));
    const step = makeStep();
    const sbo3l = new SBO3LClient({ endpoint: ENDPOINT });
    const r1 = await gateAprp(step, sbo3l, aprpFixture);
    // Simulate Inngest replaying — the handler doesn't re-run; cached value returns.
    // Switch the mock to a different response to prove replay vs re-fetch:
    fetchOnce(allow("evt-different-retry"));
    const r2 = await gateAprp(step, sbo3l, aprpFixture);
    expect(r1.audit_event_id).toBe("evt-original");
    expect(r2.audit_event_id).toBe("evt-original"); // replay, not refetch
    // Same step id was hit twice (Inngest convention) but only one fetch happened.
    expect(step.calls).toHaveLength(2);
  });

  it("stepIdPrefix override flows through", async () => {
    fetchOnce(allow());
    const step = makeStep();
    const sbo3l = new SBO3LClient({ endpoint: ENDPOINT });
    await gateAprp(step, sbo3l, aprpFixture, { stepIdPrefix: "custom.prefix" });
    expect(step.calls[0]?.id).toBe(`custom.prefix:${aprpFixture.task_id}`);
  });

  it("idempotencyKey callback is invoked", async () => {
    fetchOnce(allow());
    const step = makeStep();
    const sbo3l = new SBO3LClient({ endpoint: ENDPOINT });
    const seen: string[] = [];
    await gateAprp(step, sbo3l, aprpFixture, {
      idempotencyKey: (a) => {
        const k = `key-${a.task_id}`;
        seen.push(k);
        return k;
      },
    });
    expect(seen).toEqual([`key-${aprpFixture.task_id}`]);
  });

  it("transport errors propagate (no swallow) so Inngest retries", async () => {
    globalThis.fetch = vi.fn().mockRejectedValue(new TypeError("network down"));
    const step = makeStep();
    const sbo3l = new SBO3LClient({ endpoint: ENDPOINT });
    await expect(gateAprp(step, sbo3l, aprpFixture)).rejects.toThrow();
  });
});

describe("gateAprpSafe (no-throw wrapper)", () => {
  it("returns ok:true + receipt on allow", async () => {
    fetchOnce(allow("evt-safe-1"));
    const step = makeStep();
    const sbo3l = new SBO3LClient({ endpoint: ENDPOINT });
    const r = await gateAprpSafe(step, sbo3l, aprpFixture);
    expect(r.ok).toBe(true);
    if (r.ok) expect(r.receipt.audit_event_id).toBe("evt-safe-1");
  });

  it("returns ok:false envelope on deny (does NOT throw)", async () => {
    fetchOnce(deny("policy.deny_recipient_not_allowlisted", "evt-safe-deny"));
    const step = makeStep();
    const sbo3l = new SBO3LClient({ endpoint: ENDPOINT });
    const r = await gateAprpSafe(step, sbo3l, aprpFixture);
    expect(r.ok).toBe(false);
    if (!r.ok) {
      expect(r.decision).toBe("deny");
      expect(r.deny_code).toBe("policy.deny_recipient_not_allowlisted");
      expect(r.audit_event_id).toBe("evt-safe-deny");
    }
  });

  it("transport errors STILL throw (no-throw is policy-only, not transport)", async () => {
    globalThis.fetch = vi.fn().mockRejectedValue(new TypeError("network down"));
    const step = makeStep();
    const sbo3l = new SBO3LClient({ endpoint: ENDPOINT });
    await expect(gateAprpSafe(step, sbo3l, aprpFixture)).rejects.toThrow();
  });
});
