/**
 * `@sbo3l/0g-storage` — lightweight TypeScript client for 0G Storage.
 *
 * Why this exists: the official `@0glabs/0g-ts-sdk` ships with native
 * bindings + a heavy dep tree, and the Galileo testnet indexer is
 * documented-flaky (faucet down, 5xx mid-upload, KV nodes intermittent).
 * Production code calling the SDK directly hangs for tens of seconds
 * before the operator sees an error, which kills the demo loop.
 *
 * This wrapper exposes the same upload-and-get-rootHash flow but:
 *
 *   1. Speaks the indexer's HTTP API directly (POST /file/upload) so
 *      there's no native-binding dep. Works in Node, edge runtimes,
 *      and the browser.
 *   2. Hard 5-second per-attempt timeout (operator-overridable). The
 *      worst-case wall-clock with retries is ~17s before the caller
 *      gets a fallback-URL pointer.
 *   3. Browser fallback: if the indexer probe fails, returns the
 *      storagescan-galileo tool URL so the user can drop the same
 *      file there manually and copy back the rootHash. Honest about
 *      the gap — no hidden retry that succeeds 30 seconds later.
 *   4. Optional signed manifest: pass a signer (any function from
 *      `(message: Uint8Array) => Promise<Uint8Array>`) and the
 *      returned manifest is Ed25519-signed over the rootHash +
 *      uploaded_at + endpoint, so downstream consumers can verify
 *      the upload happened against this specific indexer at this
 *      specific time without trusting the indexer's reply alone.
 *
 * ## Usage in 5 lines (Node)
 *
 * ```ts
 * import { ZeroGStorageClient } from "@sbo3l/0g-storage";
 * const client = new ZeroGStorageClient();
 * const { rootHash, manifest } = await client.upload(payload);
 * console.log(rootHash);          // 0xc0ffee...
 * console.log(manifest.endpoint); // https://indexer-storage-testnet-turbo.0g.ai
 * ```
 *
 * ## Browser-side liveness probe
 *
 * ```ts
 * const probe = await client.probe();
 * if (!probe.live) {
 *   // Open the manual fallback. Same URL used in
 *   // apps/marketing's ZeroGUploader runtime.
 *   window.open(client.getFallbackUrl());
 * }
 * ```
 */

/**
 * Default 0G Galileo testnet indexer. Same URL the Rust backend
 * (`crates/sbo3l-storage/src/zerog_backend.rs`) and the marketing
 * uploader runtime use.
 */
export const DEFAULT_INDEXER_URL =
  "https://indexer-storage-testnet-turbo.0g.ai";

/**
 * Manual-upload tool URL. Linked from error paths so a flaky-indexer
 * day still gives the operator a recovery route.
 */
export const FALLBACK_TOOL_URL = "https://storagescan-galileo.0g.ai/tool";

/**
 * File-permalink template. `{rootHash}` is substituted with the
 * lowercase 0x-prefixed root hash returned by the indexer.
 */
export const FILE_PERMALINK_TEMPLATE =
  "https://storagescan-galileo.0g.ai/file/{rootHash}";

/**
 * Default per-attempt HTTP timeout in milliseconds. Five seconds
 * matches the marketing uploader's probe timeout — short enough that
 * a flaky-indexer day fails fast; long enough that a healthy testnet
 * succeeds on the first try.
 */
export const DEFAULT_TIMEOUT_MS = 5_000;

/**
 * Default retry delays. Three attempts at 1s + 3s match the Rust
 * backend's retry schedule (worst case 5 + 1 + 5 + 3 + 5 = 19s
 * before fallback).
 */
export const DEFAULT_RETRY_DELAYS_MS: number[] = [1_000, 3_000];

export interface ZeroGStorageClientOptions {
  /** Indexer base URL. Defaults to `DEFAULT_INDEXER_URL`. */
  endpoint?: string;
  /** Per-attempt HTTP timeout (ms). Defaults to 5000. */
  timeoutMs?: number;
  /**
   * Inter-attempt delays (ms). Length defines retry count:
   * `delays.length + 1` total attempts. Defaults to `[1000, 3000]`
   * (3 attempts).
   */
  retryDelaysMs?: number[];
  /**
   * Override the global `fetch` (e.g. in older Node versions or to
   * inject a mock for tests). Defaults to globalThis.fetch.
   */
  fetch?: typeof fetch;
}

export interface UploadOptions {
  /**
   * Chunk size in bytes. Hint to the indexer for cost-effective
   * storage; the 0G testnet currently honours 1 MiB chunks. Default
   * 1 MiB. Negative or zero values are ignored (indexer uses its
   * own default).
   */
  chunkSize?: number;
  /**
   * Replication factor. Hint to the indexer; how many storage nodes
   * should hold a replica. Default 3. Negative or zero values are
   * ignored.
   */
  replicationFactor?: number;
  /**
   * Optional signer. If supplied, the returned manifest is
   * Ed25519-signed over `<rootHash>|<uploaded_at>|<endpoint>`.
   * `signer.publicKey` must be a 32-byte Ed25519 public key
   * (base16 or raw bytes). The signature lets a downstream verifier
   * confirm the upload was witnessed by this signer at this time
   * without trusting the indexer's reply.
   */
  signer?: ManifestSigner;
}

export interface ManifestSigner {
  /** 32-byte Ed25519 public key, hex-encoded with optional `0x` prefix. */
  publicKey: string;
  /** Sign `message` and return the 64-byte Ed25519 signature. */
  sign: (message: Uint8Array) => Promise<Uint8Array> | Uint8Array;
}

export interface UploadManifest {
  rootHash: string;
  uploaded_at: string;
  endpoint: string;
  /** Lowercase 0x-prefixed signer pubkey (32 bytes hex). Empty if unsigned. */
  signer_pubkey: string;
  /** Lowercase 0x-prefixed signature over `<rootHash>|<uploaded_at>|<endpoint>`. Empty if unsigned. */
  signature: string;
  permalink: string;
}

export interface UploadResult {
  rootHash: string;
  manifest: UploadManifest;
}

export interface ProbeResult {
  live: boolean;
  latencyMs: number | null;
  status: number | null;
  /** Reason the probe failed (transport error, non-2xx, timeout). Empty when live. */
  reason: string;
}

export class ZeroGStorageError extends Error {
  /** `"timeout" | "transport" | "indexer" | "malformed-response"` */
  readonly kind: string;
  readonly attempts: number;
  readonly fallbackUrl: string;
  constructor(
    kind: string,
    attempts: number,
    detail: string,
    fallbackUrl: string,
  ) {
    super(
      `0G Storage upload failed after ${attempts} attempt(s): ${kind} (${detail}). ` +
        `Indexer is documented-flaky; fall back to ${fallbackUrl} for a manual upload.`,
    );
    this.name = "ZeroGStorageError";
    this.kind = kind;
    this.attempts = attempts;
    this.fallbackUrl = fallbackUrl;
  }
}

/**
 * Five-line client for 0G Storage. See module docs for usage.
 */
export class ZeroGStorageClient {
  private readonly endpoint: string;
  private readonly timeoutMs: number;
  private readonly retryDelaysMs: number[];
  private readonly fetchImpl: typeof fetch;

  constructor(options: ZeroGStorageClientOptions = {}) {
    this.endpoint = stripTrailingSlash(options.endpoint ?? DEFAULT_INDEXER_URL);
    this.timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;
    this.retryDelaysMs = options.retryDelaysMs ?? DEFAULT_RETRY_DELAYS_MS.slice();
    const f = options.fetch ?? globalThis.fetch;
    if (typeof f !== "function") {
      throw new Error(
        "@sbo3l/0g-storage: globalThis.fetch is undefined. Pass options.fetch in older Node versions.",
      );
    }
    this.fetchImpl = f;
  }

  /** Total attempts (first + retries) the configured policy will make. */
  maxAttempts(): number {
    return this.retryDelaysMs.length + 1;
  }

  /** Indexer base URL, post-trailing-slash-strip. */
  getEndpoint(): string {
    return this.endpoint;
  }

  /** Manual fallback URL for the storagescan-galileo upload tool. */
  getFallbackUrl(): string {
    return FALLBACK_TOOL_URL;
  }

  /** File permalink for a previously-uploaded rootHash. */
  permalinkFor(rootHash: string): string {
    return FILE_PERMALINK_TEMPLATE.replace("{rootHash}", rootHash);
  }

  /**
   * Lightweight liveness probe. Issues a HEAD-like fetch against the
   * indexer and returns the latency + a reason field. Used by browser
   * UIs to decide whether to attempt an upload at all (the marketing
   * site's ZeroGUploader runtime uses this exact pattern).
   *
   * Honours `timeoutMs` — never hangs longer than that.
   */
  async probe(): Promise<ProbeResult> {
    const url = `${this.endpoint}/`;
    const ctrl = new AbortController();
    const timer = setTimeout(() => ctrl.abort(), this.timeoutMs);
    const t0 = Date.now();
    try {
      const resp = await this.fetchImpl(url, {
        method: "GET",
        signal: ctrl.signal,
      });
      const latencyMs = Date.now() - t0;
      if (resp.ok) {
        return { live: true, latencyMs, status: resp.status, reason: "" };
      }
      return {
        live: false,
        latencyMs,
        status: resp.status,
        reason: `non-2xx: ${resp.status}`,
      };
    } catch (e) {
      const reason =
        ctrl.signal.aborted ? "timeout" : `transport: ${asString(e)}`;
      return {
        live: false,
        latencyMs: null,
        status: null,
        reason,
      };
    } finally {
      clearTimeout(timer);
    }
  }

  /**
   * Upload a payload and return its rootHash + a signed manifest.
   *
   * Retries up to `retryDelaysMs.length + 1` times with the configured
   * inter-attempt delays. Each attempt has the configured `timeoutMs`
   * upper bound. On terminal failure throws `ZeroGStorageError` whose
   * `fallbackUrl` field carries the manual-upload tool URL.
   */
  async upload(
    payload: Uint8Array,
    options: UploadOptions = {},
  ): Promise<UploadResult> {
    if (payload.length === 0) {
      throw new Error("@sbo3l/0g-storage: cannot upload empty payload");
    }

    const url = `${this.endpoint}/file/upload`;
    const max = this.maxAttempts();
    let lastDetail = "";
    let lastKind: "timeout" | "transport" | "indexer" | "malformed-response" =
      "transport";

    for (let attempt = 0; attempt < max; attempt++) {
      if (attempt > 0) {
        const delay = this.retryDelaysMs[attempt - 1] ?? 0;
        if (delay > 0) {
          await sleep(delay);
        }
      }

      const ctrl = new AbortController();
      const timer = setTimeout(() => ctrl.abort(), this.timeoutMs);
      try {
        const headers: Record<string, string> = {
          "content-type": "application/octet-stream",
        };
        if (options.chunkSize !== undefined && options.chunkSize > 0) {
          headers["x-0g-chunk-size"] = String(options.chunkSize);
        }
        if (
          options.replicationFactor !== undefined &&
          options.replicationFactor > 0
        ) {
          headers["x-0g-replication-factor"] = String(options.replicationFactor);
        }
        // Use the Uint8Array's underlying ArrayBuffer slice — both
        // Node fetch (undici) and browser fetch accept ArrayBuffer
        // as `BodyInit`. Passing the typed array directly trips
        // TS-strict's `BodyInit` check on @types/node.
        const body = payload.buffer.slice(
          payload.byteOffset,
          payload.byteOffset + payload.byteLength,
        ) as ArrayBuffer;
        const resp = await this.fetchImpl(url, {
          method: "POST",
          headers,
          body,
          signal: ctrl.signal,
        });
        if (!resp.ok) {
          lastKind = "indexer";
          lastDetail = `HTTP ${resp.status}`;
          continue;
        }
        const text = await resp.text();
        let parsed: { rootHash?: string; root_hash?: string };
        try {
          parsed = JSON.parse(text);
        } catch (e) {
          throw new ZeroGStorageError(
            "malformed-response",
            attempt + 1,
            `indexer 200 not JSON: ${asString(e)}: ${truncate(text, 120)}`,
            FALLBACK_TOOL_URL,
          );
        }
        const rootHash = parsed.rootHash ?? parsed.root_hash;
        if (typeof rootHash !== "string" || rootHash.length === 0) {
          throw new ZeroGStorageError(
            "malformed-response",
            attempt + 1,
            `indexer 200 missing rootHash: ${truncate(text, 120)}`,
            FALLBACK_TOOL_URL,
          );
        }
        const uploadedAt = new Date().toISOString();
        const manifestArgs: BuildManifestArgs = {
          rootHash,
          uploadedAt,
          endpoint: this.endpoint,
        };
        if (options.signer) {
          manifestArgs.signer = options.signer;
        }
        const manifest = await buildManifest(manifestArgs);
        return { rootHash, manifest };
      } catch (e) {
        if (e instanceof ZeroGStorageError) {
          throw e;
        }
        if (ctrl.signal.aborted) {
          lastKind = "timeout";
          lastDetail = `aborted after ${this.timeoutMs}ms`;
        } else {
          lastKind = "transport";
          lastDetail = asString(e);
        }
      } finally {
        clearTimeout(timer);
      }
    }
    throw new ZeroGStorageError(lastKind, max, lastDetail, FALLBACK_TOOL_URL);
  }
}

interface BuildManifestArgs {
  rootHash: string;
  uploadedAt: string;
  endpoint: string;
  signer?: ManifestSigner;
}

async function buildManifest(args: BuildManifestArgs): Promise<UploadManifest> {
  const permalink = FILE_PERMALINK_TEMPLATE.replace("{rootHash}", args.rootHash);
  if (!args.signer) {
    return {
      rootHash: args.rootHash,
      uploaded_at: args.uploadedAt,
      endpoint: args.endpoint,
      signer_pubkey: "",
      signature: "",
      permalink,
    };
  }
  const message = new TextEncoder().encode(
    `${args.rootHash}|${args.uploadedAt}|${args.endpoint}`,
  );
  const sig = await args.signer.sign(message);
  return {
    rootHash: args.rootHash,
    uploaded_at: args.uploadedAt,
    endpoint: args.endpoint,
    signer_pubkey: normaliseHex(args.signer.publicKey),
    signature: `0x${bytesToHex(sig)}`,
    permalink,
  };
}

function stripTrailingSlash(s: string): string {
  return s.endsWith("/") ? s.slice(0, -1) : s;
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function asString(e: unknown): string {
  if (e instanceof Error) return e.message;
  if (typeof e === "string") return e;
  return JSON.stringify(e);
}

function truncate(s: string, n: number): string {
  return s.length <= n ? s : `${s.slice(0, n)}…`;
}

function bytesToHex(bytes: Uint8Array): string {
  let out = "";
  for (const byte of bytes) {
    out += byte.toString(16).padStart(2, "0");
  }
  return out;
}

function normaliseHex(s: string): string {
  const lower = s.toLowerCase();
  return lower.startsWith("0x") ? lower : `0x${lower}`;
}
