import { createHmac } from "node:crypto";
import { describe, expect, it } from "vitest";

import { verifyLinearSignature } from "../src/signature.js";

const SECRET = "test-secret-do-not-use-in-prod";

function sign(body: string): string {
  return createHmac("sha256", SECRET).update(body).digest("hex");
}

describe("verifyLinearSignature", () => {
  it("accepts a valid signature", () => {
    const body = '{"action":"update"}';
    expect(verifyLinearSignature(body, sign(body), SECRET)).toBe(true);
  });

  it("rejects a tampered body", () => {
    const body = '{"action":"update"}';
    const sig = sign(body);
    expect(verifyLinearSignature('{"action":"create"}', sig, SECRET)).toBe(false);
  });

  it("rejects a wrong secret", () => {
    const body = '{"x":1}';
    const sig = sign(body);
    expect(verifyLinearSignature(body, sig, "different-secret")).toBe(false);
  });

  it("rejects a missing signature header without throwing", () => {
    expect(verifyLinearSignature("{}", undefined, SECRET)).toBe(false);
  });

  it("rejects a wrong-length signature without throwing", () => {
    expect(verifyLinearSignature("{}", "deadbeef", SECRET)).toBe(false);
  });

  it("rejects an empty secret without throwing", () => {
    const body = '{"x":1}';
    expect(verifyLinearSignature(body, sign(body), "")).toBe(false);
  });
});
