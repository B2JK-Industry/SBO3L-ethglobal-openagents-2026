/**
 * SBO3L gRPC client (TypeScript).
 *
 * Thin wrapper around `@grpc/grpc-js` + `@grpc/proto-loader`. Loads the
 * proto at runtime from the vendored `proto/sbo3l.proto` (shipped in
 * the npm package) and exposes a typed `createClient()` factory.
 *
 * Why not pre-generate code with `protoc-gen-grpc-js`?
 *   * The generator emits a tangle of `.js` + `.d.ts` files that need
 *     a build step to ship. Runtime proto-loader is one file, one
 *     codepath, and identical wire shape.
 *   * For three RPCs (Decide / Health / AuditChainStream) the dynamic
 *     overhead (~5ms one-time at startup) is irrelevant.
 *
 * If a downstream consumer wants static codegen, the proto file is
 * vendored at `node_modules/@sbo3l/grpc-client/proto/sbo3l.proto`
 * and can be passed straight to `protoc-gen-grpc-js`.
 */

import { credentials, type ChannelCredentials } from '@grpc/grpc-js';
import * as grpc from '@grpc/grpc-js';
import * as protoLoader from '@grpc/proto-loader';
import { fileURLToPath } from 'node:url';
import * as path from 'node:path';

// Locate the vendored proto file relative to this module. Works both
// when the package is consumed as `@sbo3l/grpc-client` (dist/index.js
// at <pkg>/dist/) and when it's hoisted to a monorepo root.
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROTO_PATH = path.resolve(__dirname, '..', 'proto', 'sbo3l.proto');

// ---------------------------------------------------------------------
// Wire types — mirror `proto/sbo3l.proto` exactly.
// ---------------------------------------------------------------------

/**
 * Mirrors the `PaymentStatus` enum in proto. Numeric values must match
 * the proto definition; the names are a convenience for branching.
 */
export enum PaymentStatus {
  PAYMENT_STATUS_UNSPECIFIED = 0,
  PAYMENT_STATUS_AUTO_APPROVED = 1,
  PAYMENT_STATUS_REJECTED = 2,
  PAYMENT_STATUS_REQUIRES_HUMAN = 3,
}

/** Mirrors the `Decision` enum in proto. */
export enum Decision {
  DECISION_UNSPECIFIED = 0,
  DECISION_ALLOW = 1,
  DECISION_DENY = 2,
  DECISION_REQUIRES_HUMAN = 3,
}

export interface DecideRequest {
  /** JSON-encoded APRP body. Same bytes a REST caller would POST. */
  aprp_json: string;
}

export interface DecideResponse {
  status: PaymentStatus;
  decision: Decision;
  /** Empty when decision is ALLOW. */
  deny_code: string;
  matched_rule_id: string;
  request_hash: string;
  policy_hash: string;
  audit_event_id: string;
  /**
   * JSON-encoded `PolicyReceipt`. Returned as an opaque string so the
   * canonical-JSON Ed25519 signature check on the verifier side reads
   * the exact bytes the server signed.
   */
  receipt_json: string;
}

export interface HealthRequest {}

export interface HealthResponse {
  status: string;
  version: string;
  audit_chain_head: string;
  audit_chain_length: number;
  uptime_seconds: number;
}

export interface AuditChainRequest {
  /** Zero means "from genesis"; otherwise emit only seq > since_seq. */
  since_seq: number;
  /** Zero means "server default cap" (1000). */
  limit: number;
}

export interface AuditChainEvent {
  seq: number;
  event_id: string;
  event_hash: string;
  prev_event_hash: string;
  event_type: string;
  ts: string;
}

// ---------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------

/**
 * The strongly-typed `Sbo3l` service client. Returned from
 * `createClient()`. Each method maps to one gRPC RPC; unary RPCs
 * return a Promise, server-streaming RPCs return an async iterable.
 */
export interface Sbo3lClient {
  decide(req: DecideRequest): Promise<DecideResponse>;
  health(req?: HealthRequest): Promise<HealthResponse>;
  auditChainStream(req: AuditChainRequest): AsyncIterable<AuditChainEvent>;
  /** Tear down the underlying gRPC channel. */
  close(): void;
}

/**
 * Options for `createClient()`.
 */
export interface ClientOptions {
  /** `host:port` of the gRPC server, e.g. `127.0.0.1:8731`. */
  address: string;
  /**
   * Optional credentials. Defaults to `credentials.createInsecure()`
   * for local development against the dev daemon. For production
   * callers, pass `credentials.createSsl()` or a properly configured
   * `ChannelCredentials`.
   */
  credentials?: ChannelCredentials;
}

/**
 * Connect to an SBO3L gRPC daemon.
 *
 * @example
 * ```ts
 * import { createClient, PaymentStatus } from '@sbo3l/grpc-client';
 *
 * const client = createClient({ address: '127.0.0.1:8731' });
 * try {
 *   const aprp = JSON.stringify({ ... });
 *   const resp = await client.decide({ aprp_json: aprp });
 *   if (resp.status === PaymentStatus.PAYMENT_STATUS_AUTO_APPROVED) {
 *     console.log('approved; receipt=', resp.receipt_json);
 *   }
 * } finally {
 *   client.close();
 * }
 * ```
 */
export function createClient(opts: ClientOptions): Sbo3lClient {
  const packageDef = protoLoader.loadSync(PROTO_PATH, {
    keepCase: true,
    longs: Number,
    enums: Number,
    defaults: true,
    oneofs: true,
  });
  // The cast through `unknown` is needed because @grpc/grpc-js types
  // the loaded package as `GrpcObject` — a generic record that doesn't
  // describe individual services. We cast to a narrower type that
  // matches our proto's namespace structure (`sbo3l.v1.Sbo3l`).
  const root = grpc.loadPackageDefinition(packageDef) as unknown as {
    sbo3l: { v1: { Sbo3l: grpc.ServiceClientConstructor } };
  };
  const ServiceCtor = root.sbo3l.v1.Sbo3l;
  const creds = opts.credentials ?? credentials.createInsecure();
  const raw = new ServiceCtor(opts.address, creds);

  return {
    decide(req: DecideRequest): Promise<DecideResponse> {
      return new Promise((resolve, reject) => {
        // The dynamic-loader mode emits camelCase aliases for snake_case
        // fields by default; we passed `keepCase: true` to preserve the
        // proto's snake_case so our TS types match the proto exactly.
        (raw as unknown as Record<string, Function>).Decide(
          req,
          (err: Error | null, resp: DecideResponse) => {
            if (err) reject(err);
            else resolve(resp);
          },
        );
      });
    },
    health(req: HealthRequest = {}): Promise<HealthResponse> {
      return new Promise((resolve, reject) => {
        (raw as unknown as Record<string, Function>).Health(
          req,
          (err: Error | null, resp: HealthResponse) => {
            if (err) reject(err);
            else resolve(resp);
          },
        );
      });
    },
    auditChainStream(req: AuditChainRequest): AsyncIterable<AuditChainEvent> {
      const call = (raw as unknown as Record<string, Function>).AuditChainStream(req) as
        & NodeJS.ReadableStream
        & { on(event: 'data', cb: (e: AuditChainEvent) => void): void; on(event: 'error', cb: (e: Error) => void): void; on(event: 'end', cb: () => void): void };
      return {
        [Symbol.asyncIterator](): AsyncIterator<AuditChainEvent> {
          // Buffer-based async iterator so we can yield events as
          // they arrive and propagate end-of-stream / errors cleanly.
          const queue: AuditChainEvent[] = [];
          const errors: Error[] = [];
          let done = false;
          let resolveNext: ((v: IteratorResult<AuditChainEvent>) => void) | null = null;

          const tryFlush = () => {
            if (!resolveNext) return;
            if (errors.length > 0) {
              const r = resolveNext;
              resolveNext = null;
              r(Promise.reject(errors.shift()!) as unknown as IteratorResult<AuditChainEvent>);
              return;
            }
            if (queue.length > 0) {
              const r = resolveNext;
              resolveNext = null;
              r({ value: queue.shift()!, done: false });
              return;
            }
            if (done) {
              const r = resolveNext;
              resolveNext = null;
              r({ value: undefined, done: true });
            }
          };

          call.on('data', (event: AuditChainEvent) => {
            queue.push(event);
            tryFlush();
          });
          call.on('error', (err: Error) => {
            errors.push(err);
            tryFlush();
          });
          call.on('end', () => {
            done = true;
            tryFlush();
          });

          return {
            next(): Promise<IteratorResult<AuditChainEvent>> {
              return new Promise((resolve, reject) => {
                resolveNext = (r) => {
                  if (r instanceof Promise) {
                    r.then(resolve, reject);
                  } else {
                    resolve(r);
                  }
                };
                tryFlush();
              });
            },
          };
        },
      };
    },
    close(): void {
      raw.close();
    },
  };
}
