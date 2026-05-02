import { describe, expect, it } from "vitest";

import { buildAprpForStep } from "../src/steps.js";

describe("buildAprpForStep", () => {
  it("uses chain=base + recipient=0x1111...1111 (matches reference policy allow rule)", () => {
    const a = buildAprpForStep({ framework: "x", intent: "purchase_api_call", amount: "0.05", step: 1 });
    expect(a.chain).toBe("base");
    expect((a.destination as { expected_recipient?: string }).expected_recipient).toBe(
      "0x1111111111111111111111111111111111111111",
    );
  });

  it("emits a fresh nonce per call", () => {
    const a = buildAprpForStep({ framework: "x", intent: "purchase_api_call", amount: "0.05", step: 1 });
    const b = buildAprpForStep({ framework: "x", intent: "purchase_api_call", amount: "0.05", step: 1 });
    expect(a.nonce).not.toBe(b.nonce);
  });

  it("expiry is 5 minutes ahead", () => {
    const before = Date.now();
    const a = buildAprpForStep({ framework: "x", intent: "purchase_api_call", amount: "0.05", step: 1 });
    const expiry = Date.parse(a.expiry);
    expect(expiry - before).toBeGreaterThanOrEqual(4 * 60 * 1000);
    expect(expiry - before).toBeLessThanOrEqual(6 * 60 * 1000);
  });

  it("task_id includes both step and framework for traceability", () => {
    const a = buildAprpForStep({ framework: "vercel-ai", intent: "purchase_api_call", amount: "0.05", step: 6 });
    expect(a.task_id).toContain("step-6");
    expect(a.task_id).toContain("vercel-ai");
  });

  it("provider_url segments per framework so policy can match per step class", () => {
    const a = buildAprpForStep({ framework: "uniswap", intent: "purchase_api_call", amount: "0.05", step: 8 });
    expect((a.destination as { url?: string }).url).toContain("/v1/uniswap");
  });
});
