/**
 * Wire shape for `GET /v1/admin/metrics`.
 *
 * The endpoint isn't shipped on `sbo3l-server` yet — this file documents
 * the expected shape so the dashboard renders against a mock by default
 * and can flip to a real daemon by setting `?endpoint=<url>` on the page.
 *
 * Server-side, the endpoint should aggregate counters from the existing
 * Prometheus instrumentation (`sbo3l_decisions_total`,
 * `sbo3l_decision_duration_seconds_bucket`, `sbo3l_audit_events_total`)
 * over a tail-window (e.g. last 60 minutes) and return one
 * `MetricsSnapshot` per minute bucket.
 */

export interface MetricsBucket {
  /** RFC 3339 timestamp at the START of the bucket window. */
  ts: string;
  /** Requests received during the bucket window (any decision). */
  requests: number;
  /** Subset that resolved as `decision: allow`. */
  allows: number;
  /** Subset that resolved as `decision: deny`. */
  denies: number;
  /** Subset that resolved as `decision: requires_human`. */
  requires_human: number;
  /** Cumulative audit-chain length at the END of the bucket window. */
  audit_chain_length: number;
  /** Latency percentiles in milliseconds for requests in this bucket. */
  latency_ms: {
    p50: number;
    p95: number;
    p99: number;
  };
}

export interface MetricsSnapshot {
  /** Window in seconds each bucket represents (60 = 1 minute). */
  bucket_seconds: number;
  /** Buckets in chronological order, oldest first. */
  buckets: MetricsBucket[];
  /** Optional: free-form description of the daemon (host, version). */
  daemon: {
    endpoint: string;
    version?: string;
    started_at?: string;
  };
}

/**
 * Fetch a snapshot from `/v1/admin/metrics`. Returns `undefined` and
 * logs a warning if the endpoint is unreachable / non-200; the caller
 * is expected to fall back to mock data.
 *
 * Note: the endpoint URL excludes the `/v1/admin/metrics` suffix —
 * `loadMetrics("http://localhost:8730")` hits
 * `http://localhost:8730/v1/admin/metrics`.
 */
export async function loadMetrics(
  endpoint: string,
  fetchImpl: typeof fetch = globalThis.fetch,
): Promise<MetricsSnapshot | undefined> {
  try {
    const r = await fetchImpl(`${endpoint.replace(/\/$/, "")}/v1/admin/metrics`, {
      headers: { Accept: "application/json" },
    });
    if (!r.ok) {
      console.warn(`/v1/admin/metrics: HTTP ${r.status}`);
      return undefined;
    }
    return (await r.json()) as MetricsSnapshot;
  } catch (e) {
    console.warn(`/v1/admin/metrics: ${e instanceof Error ? e.message : String(e)}`);
    return undefined;
  }
}

/** Derived series: requests-per-second from the bucket count + window. */
export function requestsPerSecond(snap: MetricsSnapshot): Array<{ ts: string; rps: number }> {
  return snap.buckets.map((b) => ({
    ts: b.ts,
    rps: Math.round((b.requests / snap.bucket_seconds) * 100) / 100,
  }));
}

/**
 * Derived series: percent of requests that allowed, for each bucket.
 * Returns 0 when bucket has no requests (avoids NaN flicker on idle).
 */
export function allowRatio(snap: MetricsSnapshot): Array<{ ts: string; pct: number }> {
  return snap.buckets.map((b) => ({
    ts: b.ts,
    pct: b.requests === 0 ? 0 : Math.round((b.allows / b.requests) * 1000) / 10,
  }));
}

export function denyRatio(snap: MetricsSnapshot): Array<{ ts: string; pct: number }> {
  return snap.buckets.map((b) => ({
    ts: b.ts,
    pct:
      b.requests === 0
        ? 0
        : Math.round(((b.denies + b.requires_human) / b.requests) * 1000) / 10,
  }));
}

/** Cumulative audit chain length over time, plus per-bucket delta. */
export function auditChainSeries(
  snap: MetricsSnapshot,
): Array<{ ts: string; length: number; delta: number }> {
  let prev = snap.buckets[0]?.audit_chain_length ?? 0;
  return snap.buckets.map((b) => {
    const delta = b.audit_chain_length - prev;
    prev = b.audit_chain_length;
    return { ts: b.ts, length: b.audit_chain_length, delta: Math.max(0, delta) };
  });
}

export function latencyPercentiles(
  snap: MetricsSnapshot,
): Array<{ ts: string; p50: number; p95: number; p99: number }> {
  return snap.buckets.map((b) => ({
    ts: b.ts,
    p50: b.latency_ms.p50,
    p95: b.latency_ms.p95,
    p99: b.latency_ms.p99,
  }));
}

/**
 * Format an RFC 3339 timestamp as a short HH:MM label for chart axes.
 * Falls back to the raw string on parse failure.
 */
export function shortTimeLabel(ts: string): string {
  const d = new Date(ts);
  if (Number.isNaN(d.getTime())) return ts;
  return d.toISOString().slice(11, 16); // "HH:MM"
}
