// Thin fetch wrapper around the SBO3L daemon HTTP API.
//
// Endpoints used here:
//   GET  /v1/audit?agent_pubkey=…&cursor=…&limit=…
//   POST /v1/passport/run            { request_hash } -> capsule JSON
//   GET  /health                     daemon liveness
//   WS   /v1/events                  consumed by RecentDecisionsLive
//
// All routes that hit the daemon use server-component async fetch so we
// never expose AUTH_GITHUB_SECRET / SBO3L_DAEMON_URL to the browser.
//
// Failure mode: the daemon may be unreachable in local dev. Each
// fetcher returns DaemonError instead of throwing; callers fall back
// to mock fixtures (lib/mock-data.ts) and surface the demo-mode banner.

const DAEMON_URL = process.env.SBO3L_DAEMON_URL ?? "http://localhost:8080";
const FETCH_TIMEOUT_MS = 3000;

export interface AuditEvent {
  event_id: string;
  ts_unix_ms: number;
  event_type: "policy.decision" | "audit.checkpoint";
  agent_id: string;
  agent_pubkey: string;
  decision?: "allow" | "deny";
  deny_code?: string;
  request_hash: string;
  prev_event_hash: string;
}

export interface AuditPage {
  events: AuditEvent[];
  next_cursor: string | null;
  chain_length: number;
  chain_root: string;
}

export interface PassportCapsule {
  version: "sbo3l.passport_capsule.v2";
  capsule_id: string;
  agent_id: string;
  request_hash: string;
  policy_receipt: { decision: "allow" | "deny"; signature: string };
  size_bytes: number;
  emitted_at: string;
}

export class DaemonError extends Error {
  constructor(public status: number, message: string) {
    super(message);
    this.name = "DaemonError";
  }
}

async function fetchDaemon<T>(path: string, init?: RequestInit): Promise<T> {
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), FETCH_TIMEOUT_MS);
  try {
    const res = await fetch(`${DAEMON_URL}${path}`, {
      ...init,
      signal: ctrl.signal,
      headers: { "Content-Type": "application/json", ...(init?.headers ?? {}) },
      cache: "no-store",
    });
    if (!res.ok) {
      throw new DaemonError(res.status, `${res.status} ${res.statusText}`);
    }
    return (await res.json()) as T;
  } catch (err) {
    if (err instanceof DaemonError) throw err;
    throw new DaemonError(0, err instanceof Error ? err.message : "fetch failed");
  } finally {
    clearTimeout(timer);
  }
}

export async function isDaemonAlive(): Promise<boolean> {
  // Liveness is "responded with 2xx" — not "responded with valid JSON".
  // Dev 1's /v1/healthz returns either a JSON envelope OR a plain "ok"
  // text body depending on build flags; both indicate alive. Anything
  // 5xx or transport-level (network, abort, DNS) is dead.
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), FETCH_TIMEOUT_MS);
  try {
    const res = await fetch(`${DAEMON_URL}/v1/healthz`, {
      signal: ctrl.signal,
      cache: "no-store",
    });
    return res.ok;
  } catch {
    return false;
  } finally {
    clearTimeout(timer);
  }
}

export async function listAudit(opts: {
  agentPubkey?: string;
  cursor?: string;
  limit?: number;
} = {}): Promise<AuditPage> {
  const params = new URLSearchParams();
  if (opts.agentPubkey) params.set("agent_pubkey", opts.agentPubkey);
  if (opts.cursor) params.set("cursor", opts.cursor);
  params.set("limit", String(opts.limit ?? 50));
  return fetchDaemon<AuditPage>(`/v1/audit?${params.toString()}`);
}

export async function runPassport(requestHash: string): Promise<PassportCapsule> {
  return fetchDaemon<PassportCapsule>("/v1/passport/run", {
    method: "POST",
    body: JSON.stringify({ request_hash: requestHash }),
  });
}

export function eventsWebSocketUrl(): string {
  return DAEMON_URL.replace(/^http/, "ws") + "/v1/events";
}

export const daemonUrl = DAEMON_URL;
