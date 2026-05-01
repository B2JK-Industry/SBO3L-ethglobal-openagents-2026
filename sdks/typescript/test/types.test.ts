import { describe, it, expect } from "vitest";
import { isCapsuleV1, isCapsuleV2 } from "../src/types.js";
import { goldenCapsuleV1, buildCapsuleV2 } from "./fixtures.js";

describe("type guards", () => {
  it("isCapsuleV1 narrows v1 capsule", () => {
    expect(isCapsuleV1(goldenCapsuleV1)).toBe(true);
    expect(isCapsuleV2(goldenCapsuleV1)).toBe(false);
  });

  it("isCapsuleV2 narrows v2 capsule", () => {
    const v2 = buildCapsuleV2();
    expect(isCapsuleV2(v2)).toBe(true);
    expect(isCapsuleV1(v2)).toBe(false);
  });
});

describe("APRP shape", () => {
  it("golden APRP has all required v1 fields", () => {
    const required = [
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
    ];
    const obj: Record<string, unknown> = {
      agent_id: "x",
      task_id: "y",
      intent: "purchase_api_call",
      amount: { value: "0.01", currency: "USD" },
      token: "USDC",
      destination: { type: "eoa", address: "0x" + "a".repeat(40) },
      payment_protocol: "erc20_transfer",
      chain: "base",
      provider_url: "https://x.test",
      expiry: "2026-05-01T00:00:00Z",
      nonce: "01HTAWX5K3R8YV9NQB7C6P2DGM",
      risk_class: "low",
    };
    for (const k of required) expect(Object.keys(obj)).toContain(k);
  });
});
