import { describe, it, expect, vi } from "vitest";
import {
  ZeroGStorageClient,
  ZeroGStorageError,
  DEFAULT_INDEXER_URL,
  FALLBACK_TOOL_URL,
} from "../src/index.js";

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

describe("@sbo3l/0g-storage", () => {
  it("rejects empty payload", async () => {
    const client = new ZeroGStorageClient({
      fetch: vi.fn() as unknown as typeof fetch,
    });
    await expect(client.upload(new Uint8Array(0))).rejects.toThrow(
      /empty payload/,
    );
  });

  it("happy path returns rootHash + unsigned manifest", async () => {
    const fetchMock = vi
      .fn<Parameters<typeof fetch>, ReturnType<typeof fetch>>()
      .mockResolvedValueOnce(jsonResponse({ rootHash: "0xc0ffee" }));
    const client = new ZeroGStorageClient({ fetch: fetchMock as unknown as typeof fetch });
    const out = await client.upload(new TextEncoder().encode("payload"));
    expect(out.rootHash).toBe("0xc0ffee");
    expect(out.manifest.rootHash).toBe("0xc0ffee");
    expect(out.manifest.endpoint).toBe(DEFAULT_INDEXER_URL);
    expect(out.manifest.signer_pubkey).toBe("");
    expect(out.manifest.signature).toBe("");
    expect(out.manifest.permalink).toBe(
      "https://storagescan-galileo.0g.ai/file/0xc0ffee",
    );
    expect(fetchMock).toHaveBeenCalledTimes(1);
    const url = fetchMock.mock.calls[0][0];
    expect(String(url)).toBe(`${DEFAULT_INDEXER_URL}/file/upload`);
  });

  it("accepts snake_case root_hash from indexer", async () => {
    const fetchMock = vi
      .fn<Parameters<typeof fetch>, ReturnType<typeof fetch>>()
      .mockResolvedValueOnce(jsonResponse({ root_hash: "0xabc123" }));
    const client = new ZeroGStorageClient({ fetch: fetchMock as unknown as typeof fetch });
    const out = await client.upload(new Uint8Array([1, 2, 3]));
    expect(out.rootHash).toBe("0xabc123");
  });

  it("forwards chunkSize + replicationFactor as headers", async () => {
    const fetchMock = vi
      .fn<Parameters<typeof fetch>, ReturnType<typeof fetch>>()
      .mockResolvedValueOnce(jsonResponse({ rootHash: "0x1" }));
    const client = new ZeroGStorageClient({ fetch: fetchMock as unknown as typeof fetch });
    await client.upload(new Uint8Array([1]), {
      chunkSize: 2_097_152,
      replicationFactor: 5,
    });
    const init = fetchMock.mock.calls[0][1] as RequestInit;
    const headers = init.headers as Record<string, string>;
    expect(headers["x-0g-chunk-size"]).toBe("2097152");
    expect(headers["x-0g-replication-factor"]).toBe("5");
  });

  it("retries on indexer 5xx then surfaces ZeroGStorageError with fallback URL", async () => {
    const fetchMock = vi
      .fn<Parameters<typeof fetch>, ReturnType<typeof fetch>>()
      .mockResolvedValue(new Response("bad gateway", { status: 502 }));
    const client = new ZeroGStorageClient({
      fetch: fetchMock as unknown as typeof fetch,
      retryDelaysMs: [0, 0],
    });
    const err = await client.upload(new Uint8Array([1])).catch((e) => e);
    expect(err).toBeInstanceOf(ZeroGStorageError);
    expect((err as ZeroGStorageError).attempts).toBe(3);
    expect((err as ZeroGStorageError).fallbackUrl).toBe(FALLBACK_TOOL_URL);
    expect((err as ZeroGStorageError).kind).toBe("indexer");
    expect(fetchMock).toHaveBeenCalledTimes(3);
  });

  it("malformed JSON 200 surfaces malformed-response immediately (no retry)", async () => {
    const fetchMock = vi
      .fn<Parameters<typeof fetch>, ReturnType<typeof fetch>>()
      .mockResolvedValue(new Response("<html>not json</html>", { status: 200 }));
    const client = new ZeroGStorageClient({
      fetch: fetchMock as unknown as typeof fetch,
      retryDelaysMs: [0, 0],
    });
    const err = await client.upload(new Uint8Array([1])).catch((e) => e);
    expect(err).toBeInstanceOf(ZeroGStorageError);
    expect((err as ZeroGStorageError).kind).toBe("malformed-response");
    // First-attempt terminal — not retried.
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("missing rootHash field in 200 surfaces malformed-response", async () => {
    const fetchMock = vi
      .fn<Parameters<typeof fetch>, ReturnType<typeof fetch>>()
      .mockResolvedValue(jsonResponse({ commitment: "0xdead" }));
    const client = new ZeroGStorageClient({
      fetch: fetchMock as unknown as typeof fetch,
      retryDelaysMs: [0, 0],
    });
    const err = await client.upload(new Uint8Array([1])).catch((e) => e);
    expect(err).toBeInstanceOf(ZeroGStorageError);
    expect((err as ZeroGStorageError).kind).toBe("malformed-response");
  });

  it("manifest is signed when signer is provided", async () => {
    const fetchMock = vi
      .fn<Parameters<typeof fetch>, ReturnType<typeof fetch>>()
      .mockResolvedValueOnce(jsonResponse({ rootHash: "0xfeed" }));
    // Stub signer: returns a deterministic 64-byte "signature"
    // (0x01 02 03 ... 40) so we can assert the encoding.
    const fakeSig = new Uint8Array(64);
    for (let i = 0; i < 64; i++) fakeSig[i] = i + 1;
    const signer = {
      publicKey: "0x" + "aa".repeat(32),
      sign: () => fakeSig,
    };
    const client = new ZeroGStorageClient({ fetch: fetchMock as unknown as typeof fetch });
    const out = await client.upload(new Uint8Array([1]), { signer });
    expect(out.manifest.signer_pubkey).toBe("0x" + "aa".repeat(32));
    expect(out.manifest.signature.length).toBe(2 + 64 * 2); // "0x" + 128 hex
    expect(out.manifest.signature.startsWith("0x")).toBe(true);
    // Spot-check signature bytes (0x01 ... 0x40 → "0102...40").
    expect(out.manifest.signature.slice(2, 6)).toBe("0102");
  });

  it("normalises hex signer pubkey (uppercase + missing 0x)", async () => {
    const fetchMock = vi
      .fn<Parameters<typeof fetch>, ReturnType<typeof fetch>>()
      .mockResolvedValueOnce(jsonResponse({ rootHash: "0xaa" }));
    const client = new ZeroGStorageClient({ fetch: fetchMock as unknown as typeof fetch });
    const out = await client.upload(new Uint8Array([1]), {
      signer: {
        publicKey: "ABCDEF" + "00".repeat(29),
        sign: () => new Uint8Array(64),
      },
    });
    expect(out.manifest.signer_pubkey.startsWith("0x")).toBe(true);
    expect(out.manifest.signer_pubkey).toBe(
      ("0xabcdef" + "00".repeat(29)).toLowerCase(),
    );
  });

  it("probe returns live=true on 200", async () => {
    const fetchMock = vi
      .fn<Parameters<typeof fetch>, ReturnType<typeof fetch>>()
      .mockResolvedValueOnce(new Response("ok", { status: 200 }));
    const client = new ZeroGStorageClient({ fetch: fetchMock as unknown as typeof fetch });
    const probe = await client.probe();
    expect(probe.live).toBe(true);
    expect(probe.status).toBe(200);
    expect(probe.reason).toBe("");
  });

  it("probe returns live=false with reason on 5xx", async () => {
    const fetchMock = vi
      .fn<Parameters<typeof fetch>, ReturnType<typeof fetch>>()
      .mockResolvedValueOnce(new Response("down", { status: 503 }));
    const client = new ZeroGStorageClient({ fetch: fetchMock as unknown as typeof fetch });
    const probe = await client.probe();
    expect(probe.live).toBe(false);
    expect(probe.status).toBe(503);
    expect(probe.reason).toMatch(/non-2xx/);
  });

  it("getFallbackUrl returns the storagescan tool URL", () => {
    const client = new ZeroGStorageClient();
    expect(client.getFallbackUrl()).toBe(FALLBACK_TOOL_URL);
  });

  it("permalinkFor builds the storagescan file URL", () => {
    const client = new ZeroGStorageClient();
    expect(client.permalinkFor("0xc0ffee")).toBe(
      "https://storagescan-galileo.0g.ai/file/0xc0ffee",
    );
  });

  it("maxAttempts matches retryDelaysMs.length + 1", () => {
    expect(new ZeroGStorageClient().maxAttempts()).toBe(3);
    expect(
      new ZeroGStorageClient({ retryDelaysMs: [] }).maxAttempts(),
    ).toBe(1);
    expect(
      new ZeroGStorageClient({ retryDelaysMs: [100, 200, 400] }).maxAttempts(),
    ).toBe(4);
  });

  it("custom endpoint strips trailing slash", () => {
    const client = new ZeroGStorageClient({
      endpoint: "https://my-indexer.example.com/",
    });
    expect(client.getEndpoint()).toBe("https://my-indexer.example.com");
  });

  it("constructor throws when fetch is unavailable", () => {
    const original = (globalThis as { fetch?: typeof fetch }).fetch;
    (globalThis as { fetch?: typeof fetch }).fetch = undefined;
    try {
      expect(() => new ZeroGStorageClient()).toThrow(/fetch is undefined/);
    } finally {
      (globalThis as { fetch?: typeof fetch }).fetch = original;
    }
  });
});
