/**
 * SBO3L HTTP client. Wraps `POST /v1/payment-requests` and `GET /v1/health`.
 *
 * Uses the runtime-provided `fetch` (Node >= 18, modern browsers). Does not
 * bundle a polyfill — peer-dep style; if you're on Node < 18, install
 * `undici` and pass it via the `fetch` option.
 */

import type {
  PaymentRequest,
  PaymentRequestResponse,
  ProblemDetail,
} from "./types.js";
import { SBO3LError, SBO3LTransportError, isProblemDetail } from "./errors.js";
import { authHeader, type AuthConfig } from "./auth.js";
import { verify, verifyOrThrow } from "./passport.js";

/** Node 18+ / browser fetch signature. */
export type FetchLike = (
  input: string | URL,
  init?: RequestInit,
) => Promise<Response>;

export interface SBO3LClientOptions {
  /** Daemon base URL. Example: `http://localhost:8730`. No trailing slash needed. */
  endpoint: string;

  /**
   * Auth config (bearer or JWT). When omitted, requests are unauthenticated;
   * the daemon rejects with `auth.required` unless `SBO3L_ALLOW_UNAUTHENTICATED=1`.
   *
   * For convenience you may also pass a string — interpreted as a bearer token.
   */
  auth?: AuthConfig | string;

  /**
   * Override the runtime `fetch`. Pass this to use `undici`, `node-fetch`, or
   * a mock during tests. Defaults to global `fetch`.
   */
  fetch?: FetchLike;

  /**
   * Per-request timeout in milliseconds. Default 30_000. The client uses
   * `AbortController` to enforce the timeout regardless of the underlying
   * fetch implementation.
   */
  timeoutMs?: number;

  /**
   * Optional `User-Agent` suffix. The SDK always includes
   * `@sbo3l/sdk/<version>`; anything passed here is appended.
   */
  userAgent?: string;
}

export interface SubmitOptions {
  /**
   * Idempotency key for safe retry. Must be 16..64 ASCII chars. When set,
   * the daemon caches the response envelope; subsequent retries with the
   * same key + body return the cached envelope without re-running side
   * effects. Different body + same key → HTTP 409 `protocol.idempotency_conflict`.
   */
  idempotencyKey?: string;

  /** Optional `AbortSignal` to cancel the request from outside. */
  signal?: AbortSignal;

  /** Override the client default timeout for this call. */
  timeoutMs?: number;
}

const SDK_VERSION = "0.1.0";

/**
 * Top-level SDK client.
 *
 * @example
 * ```ts
 * import { SBO3LClient } from "@sbo3l/sdk";
 *
 * const client = new SBO3LClient({
 *   endpoint: "http://localhost:8730",
 *   auth: { kind: "bearer", token: process.env.SBO3L_BEARER_TOKEN! },
 * });
 *
 * const r = await client.submit(aprp);
 * if (r.decision === "allow") console.log(r.receipt.execution_ref);
 * ```
 */
export class SBO3LClient {
  readonly endpoint: string;
  readonly passport: { verify: typeof verify; verifyOrThrow: typeof verifyOrThrow };

  private readonly auth: AuthConfig;
  private readonly fetchImpl: FetchLike;
  private readonly timeoutMs: number;
  private readonly userAgent: string;

  constructor(opts: SBO3LClientOptions) {
    this.endpoint = stripTrailingSlash(opts.endpoint);
    this.auth = normalizeAuth(opts.auth);
    this.fetchImpl = opts.fetch ?? defaultFetch();
    this.timeoutMs = opts.timeoutMs ?? 30_000;
    this.userAgent = buildUserAgent(opts.userAgent);
    // Re-export passport helpers as a method-bag for ergonomics.
    this.passport = { verify, verifyOrThrow };
  }

  /**
   * Submit an APRP to `POST /v1/payment-requests` and return the response
   * envelope. Throws `SBO3LError` on a non-2xx daemon response (carries the
   * RFC 7807 problem-detail) and `SBO3LTransportError` on network failures.
   */
  async submit(
    request: PaymentRequest,
    options: SubmitOptions = {},
  ): Promise<PaymentRequestResponse> {
    const url = `${this.endpoint}/v1/payment-requests`;
    const headers: Record<string, string> = {
      "Content-Type": "application/json",
      Accept: "application/json",
      "User-Agent": this.userAgent,
    };
    const auth = await authHeader(this.auth);
    if (auth !== undefined) headers["Authorization"] = auth;
    if (options.idempotencyKey !== undefined) {
      headers["Idempotency-Key"] = options.idempotencyKey;
    }

    const body = JSON.stringify(request);
    const res = await this.doFetch(url, "POST", headers, body, options);
    return parseEnvelope(res, await readText(res));
  }

  /**
   * Hit `GET /v1/health`. Returns `true` when the daemon answers `ok`.
   * Throws `SBO3LTransportError` on connection failure.
   */
  async health(options: { signal?: AbortSignal; timeoutMs?: number } = {}): Promise<boolean> {
    const url = `${this.endpoint}/v1/health`;
    const headers: Record<string, string> = {
      Accept: "text/plain",
      "User-Agent": this.userAgent,
    };
    const res = await this.doFetch(url, "GET", headers, undefined, options);
    if (res.status !== 200) return false;
    const txt = (await readText(res)).trim();
    return txt === "ok";
  }

  private async doFetch(
    url: string,
    method: "GET" | "POST",
    headers: Record<string, string>,
    body: string | undefined,
    options: { signal?: AbortSignal; timeoutMs?: number },
  ): Promise<Response> {
    const timeout = options.timeoutMs ?? this.timeoutMs;
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(new Error("sbo3l: timeout")), timeout);
    if (options.signal !== undefined) {
      // Forward external aborts to the inner controller.
      if (options.signal.aborted) controller.abort(options.signal.reason);
      else
        options.signal.addEventListener("abort", () => controller.abort(options.signal?.reason), {
          once: true,
        });
    }
    try {
      const init: RequestInit = {
        method,
        headers,
        signal: controller.signal,
      };
      if (body !== undefined) {
        init.body = body;
      }
      return await this.fetchImpl(url, init);
    } catch (err) {
      throw new SBO3LTransportError(
        err instanceof Error ? err.message : "fetch failed",
        err,
      );
    } finally {
      clearTimeout(timer);
    }
  }
}

/* -------------------------------------------------------------------------- */

function stripTrailingSlash(s: string): string {
  return s.endsWith("/") ? s.slice(0, -1) : s;
}

function normalizeAuth(auth: AuthConfig | string | undefined): AuthConfig {
  if (auth === undefined) return { kind: "none" };
  if (typeof auth === "string") return { kind: "bearer", token: auth };
  return auth;
}

function defaultFetch(): FetchLike {
  if (typeof fetch === "undefined") {
    throw new Error(
      "global fetch is not available — pass `fetch` in client options (Node < 18 needs `undici`).",
    );
  }
  return fetch.bind(globalThis) as FetchLike;
}

function buildUserAgent(suffix: string | undefined): string {
  const base = `@sbo3l/sdk/${SDK_VERSION}`;
  return suffix !== undefined && suffix.length > 0 ? `${base} ${suffix}` : base;
}

async function readText(res: Response): Promise<string> {
  try {
    return await res.text();
  } catch (err) {
    throw new SBO3LTransportError(
      err instanceof Error ? err.message : "failed to read response body",
      err,
    );
  }
}

function parseEnvelope(res: Response, raw: string): PaymentRequestResponse {
  if (res.status === 200) {
    let parsed: unknown;
    try {
      parsed = JSON.parse(raw);
    } catch (err) {
      throw new SBO3LTransportError(
        `daemon returned 200 but body is not JSON: ${err instanceof Error ? err.message : "?"}`,
        err,
      );
    }
    return parsed as PaymentRequestResponse;
  }

  // Non-200 → expect a Problem detail body.
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    throw new SBO3LError({
      type: `https://schemas.sbo3l.dev/errors/transport.unparseable_error`,
      title: "Daemon returned non-JSON error body",
      status: res.status,
      detail: raw.slice(0, 512),
      code: "transport.unparseable_error",
    });
  }
  if (isProblemDetail(parsed)) {
    throw new SBO3LError(parsed);
  }
  // Body parsed but is not a Problem; synthesize one to keep the contract.
  const synth: ProblemDetail = {
    type: `https://schemas.sbo3l.dev/errors/transport.unexpected_error_shape`,
    title: "Daemon returned non-Problem JSON error body",
    status: res.status,
    detail: JSON.stringify(parsed).slice(0, 512),
    code: "transport.unexpected_error_shape",
  };
  throw new SBO3LError(synth);
}
