import { describe, it, expect } from "vitest";
import {
  authHeader,
  decodeJwtClaims,
  assertJwtSubMatches,
  type AuthConfig,
} from "../src/auth.js";

/** Build a JWT-shaped string with arbitrary claim payload. Signature is fake. */
function fakeJwt(claims: Record<string, unknown>): string {
  const header = base64url(JSON.stringify({ alg: "EdDSA", typ: "JWT" }));
  const payload = base64url(JSON.stringify(claims));
  const sig = "AAAA"; // not validated client-side
  return `${header}.${payload}.${sig}`;
}

function base64url(s: string): string {
  return Buffer.from(s, "utf-8")
    .toString("base64")
    .replace(/=+$/, "")
    .replace(/\+/g, "-")
    .replace(/\//g, "_");
}

describe("authHeader", () => {
  it("returns undefined for kind=none", async () => {
    const h = await authHeader({ kind: "none" });
    expect(h).toBeUndefined();
  });

  it("formats bearer header", async () => {
    const h = await authHeader({ kind: "bearer", token: "abc" });
    expect(h).toBe("Bearer abc");
  });

  it("formats jwt header (also Bearer per RFC 6750)", async () => {
    const h = await authHeader({ kind: "jwt", token: "eyJ.x.y" });
    expect(h).toBe("Bearer eyJ.x.y");
  });

  it("invokes jwt-supplier per call", async () => {
    let calls = 0;
    const cfg: AuthConfig = {
      kind: "jwt-supplier",
      supplier: async () => {
        calls += 1;
        return "tok";
      },
    };
    await authHeader(cfg);
    await authHeader(cfg);
    expect(calls).toBe(2);
  });
});

describe("decodeJwtClaims", () => {
  it("decodes a well-formed three-segment token", () => {
    const t = fakeJwt({ sub: "research-agent-01", iat: 1700000000 });
    const claims = decodeJwtClaims(t);
    expect(claims["sub"]).toBe("research-agent-01");
    expect(claims["iat"]).toBe(1700000000);
  });

  it("throws on non-three-segment input", () => {
    expect(() => decodeJwtClaims("only.two")).toThrow(/three dot-separated/);
  });

  it("throws on empty payload segment", () => {
    expect(() => decodeJwtClaims("a..c")).toThrow(/empty payload/);
  });

  it("throws on non-object payload", () => {
    const t = `${base64url("h")}.${base64url(JSON.stringify("scalar"))}.sig`;
    expect(() => decodeJwtClaims(t)).toThrow(/not a JSON object/);
  });
});

describe("assertJwtSubMatches", () => {
  it("passes when sub matches", () => {
    const t = fakeJwt({ sub: "research-agent-01" });
    expect(() => assertJwtSubMatches(t, "research-agent-01")).not.toThrow();
  });

  it("throws when sub differs", () => {
    const t = fakeJwt({ sub: "other-agent" });
    expect(() => assertJwtSubMatches(t, "research-agent-01")).toThrow(
      /does not match expected/,
    );
  });

  it("throws when sub is missing", () => {
    const t = fakeJwt({ iat: 1 });
    expect(() => assertJwtSubMatches(t, "research-agent-01")).toThrow(
      /missing or non-string 'sub'/,
    );
  });
});
