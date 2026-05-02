import { describe, expect, it, vi } from "vitest";

import {
  allowRatio,
  auditChainSeries,
  denyRatio,
  latencyPercentiles,
  loadMetrics,
  requestsPerSecond,
  shortTimeLabel,
  type MetricsSnapshot,
} from "./metrics.js";

const SAMPLE: MetricsSnapshot = {
  bucket_seconds: 60,
  daemon: { endpoint: "test" },
  buckets: [
    {
      ts: "2026-05-02T10:00:00Z",
      requests: 120,
      allows: 100,
      denies: 15,
      requires_human: 5,
      audit_chain_length: 1000,
      latency_ms: { p50: 12, p95: 40, p99: 90 },
    },
    {
      ts: "2026-05-02T10:01:00Z",
      requests: 60,
      allows: 60,
      denies: 0,
      requires_human: 0,
      audit_chain_length: 1060,
      latency_ms: { p50: 10, p95: 35, p99: 80 },
    },
    {
      ts: "2026-05-02T10:02:00Z",
      requests: 0,
      allows: 0,
      denies: 0,
      requires_human: 0,
      audit_chain_length: 1060,
      latency_ms: { p50: 0, p95: 0, p99: 0 },
    },
  ],
};

describe("requestsPerSecond", () => {
  it("derives RPS from bucket count + window seconds", () => {
    expect(requestsPerSecond(SAMPLE)).toEqual([
      { ts: "2026-05-02T10:00:00Z", rps: 2 },
      { ts: "2026-05-02T10:01:00Z", rps: 1 },
      { ts: "2026-05-02T10:02:00Z", rps: 0 },
    ]);
  });
});

describe("allowRatio + denyRatio", () => {
  it("returns percent of allowed requests per bucket", () => {
    const r = allowRatio(SAMPLE);
    expect(r[0]?.pct).toBeCloseTo(83.3, 1);
    expect(r[1]?.pct).toBe(100);
  });

  it("returns 0 (not NaN) on idle buckets", () => {
    const r = allowRatio(SAMPLE);
    expect(r[2]?.pct).toBe(0);
  });

  it("denyRatio bundles deny + requires_human (the LLM treats both as deny)", () => {
    const d = denyRatio(SAMPLE);
    // bucket 0: (15 + 5) / 120 = 16.7%
    expect(d[0]?.pct).toBeCloseTo(16.7, 1);
  });
});

describe("auditChainSeries", () => {
  it("emits cumulative length + per-bucket delta", () => {
    const s = auditChainSeries(SAMPLE);
    expect(s).toEqual([
      { ts: "2026-05-02T10:00:00Z", length: 1000, delta: 0 },
      { ts: "2026-05-02T10:01:00Z", length: 1060, delta: 60 },
      { ts: "2026-05-02T10:02:00Z", length: 1060, delta: 0 },
    ]);
  });

  it("clamps delta at 0 (chain only grows; if it shrinks something's broken)", () => {
    const broken: MetricsSnapshot = {
      ...SAMPLE,
      buckets: [
        { ...SAMPLE.buckets[0]!, audit_chain_length: 100 },
        { ...SAMPLE.buckets[1]!, audit_chain_length: 50 },
      ],
    };
    const s = auditChainSeries(broken);
    expect(s[1]?.delta).toBe(0);
  });
});

describe("latencyPercentiles", () => {
  it("re-shapes the nested latency_ms struct into a flat per-percentile row", () => {
    expect(latencyPercentiles(SAMPLE)[0]).toEqual({
      ts: "2026-05-02T10:00:00Z",
      p50: 12,
      p95: 40,
      p99: 90,
    });
  });
});

describe("shortTimeLabel", () => {
  it("formats RFC 3339 timestamps as HH:MM", () => {
    expect(shortTimeLabel("2026-05-02T10:30:00Z")).toBe("10:30");
  });

  it("returns the raw string for unparseable input", () => {
    expect(shortTimeLabel("garbage")).toBe("garbage");
  });
});

describe("loadMetrics", () => {
  it("returns the parsed snapshot on 200", async () => {
    const fetchImpl = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => SAMPLE,
    } as Response);
    const out = await loadMetrics("https://daemon", fetchImpl as never);
    expect(out).toEqual(SAMPLE);
    expect(fetchImpl).toHaveBeenCalledWith(
      "https://daemon/v1/admin/metrics",
      expect.anything(),
    );
  });

  it("strips trailing slash from endpoint", async () => {
    const fetchImpl = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => SAMPLE,
    } as Response);
    await loadMetrics("https://daemon/", fetchImpl as never);
    expect(fetchImpl).toHaveBeenCalledWith(
      "https://daemon/v1/admin/metrics",
      expect.anything(),
    );
  });

  it("returns undefined on non-200", async () => {
    const fetchImpl = vi.fn().mockResolvedValue({ ok: false, status: 500 } as Response);
    const out = await loadMetrics("https://daemon", fetchImpl as never);
    expect(out).toBeUndefined();
  });

  it("returns undefined on network error", async () => {
    const fetchImpl = vi.fn().mockRejectedValue(new TypeError("network down"));
    const out = await loadMetrics("https://daemon", fetchImpl as never);
    expect(out).toBeUndefined();
  });
});
