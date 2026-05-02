import { describe, it, expect } from "vitest";
import { SBO3LClient } from "../src/client.js";
import { SBO3LError, SBO3LTransportError } from "../src/errors.js";
import type { FetchLike } from "../src/client.js";
import type { PaymentRequestResponse, ProblemDetail } from "../src/types.js";
import { goldenAprp, goldenCapsuleV1 } from "./fixtures.js";

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

function textResponse(body: string, status = 200): Response {
  return new Response(body, {
    status,
    headers: { "Content-Type": "text/plain" },
  });
}

const goldenEnvelope: PaymentRequestResponse = {
  status: "auto_approved",
  decision: "allow",
  deny_code: null,
  matched_rule_id: "allow-low-risk-x402",
  request_hash: goldenCapsuleV1.request.request_hash,
  policy_hash: goldenCapsuleV1.policy.policy_hash,
  audit_event_id: goldenCapsuleV1.audit.audit_event_id,
  receipt: goldenCapsuleV1.decision.receipt,
};

describe("SBO3LClient construction", () => {
  it("strips trailing slash from endpoint", () => {
    const c = new SBO3LClient({
      endpoint: "http://localhost:8730/",
      auth: { kind: "none" },
      fetch: (() => Promise.resolve(jsonResponse({}))) as FetchLike,
    });
    expect(c.endpoint).toBe("http://localhost:8730");
  });

  it("accepts string auth as bearer token shorthand", async () => {
    let captured: RequestInit | undefined;
    const fakeFetch: FetchLike = async (_url, init) => {
      captured = init;
      return jsonResponse(goldenEnvelope);
    };
    const c = new SBO3LClient({
      endpoint: "http://localhost:8730",
      auth: "shorthand-token",
      fetch: fakeFetch,
    });
    await c.submit(goldenAprp);
    const headers = captured?.headers as Record<string, string>;
    expect(headers["Authorization"]).toBe("Bearer shorthand-token");
  });

  it("re-exports passport helpers on instance", () => {
    const c = new SBO3LClient({
      endpoint: "http://localhost:8730",
      fetch: (() => Promise.resolve(jsonResponse({}))) as FetchLike,
    });
    expect(typeof c.passport.verify).toBe("function");
    expect(typeof c.passport.verifyOrThrow).toBe("function");
  });
});

describe("SBO3LClient.submit — happy path", () => {
  it("calls POST /v1/payment-requests with JSON body", async () => {
    let url: string | URL = "";
    let init: RequestInit | undefined;
    const fakeFetch: FetchLike = async (u, i) => {
      url = u;
      init = i;
      return jsonResponse(goldenEnvelope);
    };
    const c = new SBO3LClient({
      endpoint: "http://localhost:8730",
      auth: { kind: "bearer", token: "tok" },
      fetch: fakeFetch,
    });

    const r = await c.submit(goldenAprp);
    expect(r.decision).toBe("allow");
    expect(String(url)).toBe("http://localhost:8730/v1/payment-requests");
    expect(init?.method).toBe("POST");
    const headers = init?.headers as Record<string, string>;
    expect(headers["Content-Type"]).toBe("application/json");
    expect(headers["Authorization"]).toBe("Bearer tok");
    expect(headers["User-Agent"]).toMatch(/^@sbo3l\/sdk\/0\.1\.0/);
    expect(typeof init?.body).toBe("string");
  });

  it("forwards Idempotency-Key when provided", async () => {
    let init: RequestInit | undefined;
    const fakeFetch: FetchLike = async (_u, i) => {
      init = i;
      return jsonResponse(goldenEnvelope);
    };
    const c = new SBO3LClient({
      endpoint: "http://localhost:8730",
      fetch: fakeFetch,
    });
    await c.submit(goldenAprp, { idempotencyKey: "0123456789abcdef0123" });
    const headers = init?.headers as Record<string, string>;
    expect(headers["Idempotency-Key"]).toBe("0123456789abcdef0123");
  });

  it("omits Authorization header when auth=none", async () => {
    let init: RequestInit | undefined;
    const fakeFetch: FetchLike = async (_u, i) => {
      init = i;
      return jsonResponse(goldenEnvelope);
    };
    const c = new SBO3LClient({
      endpoint: "http://localhost:8730",
      auth: { kind: "none" },
      fetch: fakeFetch,
    });
    await c.submit(goldenAprp);
    const headers = init?.headers as Record<string, string>;
    expect(headers["Authorization"]).toBeUndefined();
  });

  it("appends user-agent suffix when configured", async () => {
    let init: RequestInit | undefined;
    const fakeFetch: FetchLike = async (_u, i) => {
      init = i;
      return jsonResponse(goldenEnvelope);
    };
    const c = new SBO3LClient({
      endpoint: "http://localhost:8730",
      fetch: fakeFetch,
      userAgent: "research-agent/1.0",
    });
    await c.submit(goldenAprp);
    const headers = init?.headers as Record<string, string>;
    expect(headers["User-Agent"]).toBe("@sbo3l/sdk/0.1.0 research-agent/1.0");
  });
});

describe("SBO3LClient.submit — error envelopes", () => {
  it("throws SBO3LError with RFC 7807 body on 401", async () => {
    const problem: ProblemDetail = {
      type: "https://schemas.sbo3l.dev/errors/auth.required",
      title: "Authentication required",
      status: 401,
      detail: "Authorization header missing",
      code: "auth.required",
    };
    const c = new SBO3LClient({
      endpoint: "http://localhost:8730",
      fetch: (async () => jsonResponse(problem, 401)) as FetchLike,
    });
    await expect(c.submit(goldenAprp)).rejects.toBeInstanceOf(SBO3LError);
    try {
      await c.submit(goldenAprp);
    } catch (err) {
      expect(err).toBeInstanceOf(SBO3LError);
      expect((err as SBO3LError).code).toBe("auth.required");
      expect((err as SBO3LError).status).toBe(401);
    }
  });

  it("throws SBO3LError with synth body when error body is non-Problem JSON", async () => {
    const c = new SBO3LClient({
      endpoint: "http://localhost:8730",
      fetch: (async () => jsonResponse({ wat: "no" }, 500)) as FetchLike,
    });
    await expect(c.submit(goldenAprp)).rejects.toMatchObject({
      code: "transport.unexpected_error_shape",
      status: 500,
    });
  });

  it("throws SBO3LError when error body is unparseable", async () => {
    const c = new SBO3LClient({
      endpoint: "http://localhost:8730",
      fetch: (async () => textResponse("totally-not-json", 502)) as FetchLike,
    });
    await expect(c.submit(goldenAprp)).rejects.toMatchObject({
      code: "transport.unparseable_error",
      status: 502,
    });
  });

  it("throws SBO3LTransportError on network failure", async () => {
    const fakeFetch: FetchLike = async () => {
      throw new TypeError("ECONNREFUSED");
    };
    const c = new SBO3LClient({
      endpoint: "http://localhost:8730",
      fetch: fakeFetch,
    });
    await expect(c.submit(goldenAprp)).rejects.toBeInstanceOf(SBO3LTransportError);
  });

  it("throws SBO3LTransportError when 200 body is not JSON", async () => {
    const c = new SBO3LClient({
      endpoint: "http://localhost:8730",
      fetch: (async () => textResponse("not-json", 200)) as FetchLike,
    });
    await expect(c.submit(goldenAprp)).rejects.toBeInstanceOf(SBO3LTransportError);
  });
});

describe("SBO3LClient.submit — abort + timeout", () => {
  it("respects an external AbortSignal", async () => {
    const controller = new AbortController();
    const fakeFetch: FetchLike = (_u, init) =>
      new Promise((_resolve, reject) => {
        if (init?.signal?.aborted === true) {
          reject(new DOMException("aborted", "AbortError"));
          return;
        }
        init?.signal?.addEventListener(
          "abort",
          () => reject(new DOMException("aborted", "AbortError")),
          { once: true },
        );
      });
    const c = new SBO3LClient({
      endpoint: "http://localhost:8730",
      fetch: fakeFetch,
    });
    const p = c.submit(goldenAprp, { signal: controller.signal });
    controller.abort();
    await expect(p).rejects.toBeInstanceOf(SBO3LTransportError);
  });

  it("aborts on per-request timeout", async () => {
    // Use a real short timeout instead of fake timers — fake timers create
    // a race between the abort listener's synchronous reject and the outer
    // `await fetchImpl(...)` rejection-handler installation, which vitest
    // flags as an unhandled rejection. With real timers and a 20ms cap,
    // the test still finishes quickly.
    const fakeFetch: FetchLike = (_u, init) =>
      new Promise((_resolve, reject) => {
        if (init?.signal?.aborted === true) {
          reject(new DOMException("aborted", "AbortError"));
          return;
        }
        init?.signal?.addEventListener(
          "abort",
          () => reject(new DOMException("aborted", "AbortError")),
          { once: true },
        );
      });
    const c = new SBO3LClient({
      endpoint: "http://localhost:8730",
      fetch: fakeFetch,
      timeoutMs: 20,
    });
    await expect(c.submit(goldenAprp)).rejects.toBeInstanceOf(SBO3LTransportError);
  });
});

describe("SBO3LClient.health", () => {
  it("returns true when daemon answers `ok`", async () => {
    const c = new SBO3LClient({
      endpoint: "http://localhost:8730",
      fetch: (async () => textResponse("ok\n")) as FetchLike,
    });
    expect(await c.health()).toBe(true);
  });

  it("returns false on non-200", async () => {
    const c = new SBO3LClient({
      endpoint: "http://localhost:8730",
      fetch: (async () => textResponse("no", 503)) as FetchLike,
    });
    expect(await c.health()).toBe(false);
  });

  it("returns false when body is not literally `ok`", async () => {
    const c = new SBO3LClient({
      endpoint: "http://localhost:8730",
      fetch: (async () => textResponse("up")) as FetchLike,
    });
    expect(await c.health()).toBe(false);
  });
});
