import type { MetricsSnapshot } from "../lib/metrics.js";

/**
 * Synthetic 60-minute snapshot for the dashboard's default render.
 * Numbers are inspired by what a small staging daemon serving 5-15 req/s
 * with mostly-allowed traffic + occasional deny spikes might produce.
 *
 * This file is the canonical reference for the wire shape the daemon's
 * `/v1/admin/metrics` endpoint should return.
 */

const NOW_MINUTES = 60;
const START = new Date(Date.now() - NOW_MINUTES * 60 * 1000);

// Deterministic pseudo-random so the chart shape is stable across renders.
function noise(seed: number, scale: number): number {
  const x = Math.sin(seed * 12.9898) * 43758.5453;
  return (x - Math.floor(x)) * scale;
}

let chainLength = 1_240_000;

export const MOCK_METRICS: MetricsSnapshot = {
  bucket_seconds: 60,
  daemon: {
    endpoint: "(mock data — set ?endpoint=http://localhost:8730 for live)",
    version: "1.2.0",
    started_at: new Date(Date.now() - 14 * 24 * 3600 * 1000).toISOString(),
  },
  buckets: Array.from({ length: NOW_MINUTES }, (_, i) => {
    const base = 8 + Math.sin(i / 6) * 4 + noise(i, 3);
    const requests = Math.max(0, Math.round(base * 60));
    // Most buckets are mostly-allow. Inject 3 deny spikes around minute
    // 14, 32, and 47 to make the allow/deny chart visually interesting.
    const denyFloor = i === 14 || i === 32 || i === 47 ? 0.18 : 0.04;
    const denies = Math.round(requests * (denyFloor + noise(i + 1, 0.02)));
    const requiresHuman =
      i === 47 ? Math.round(requests * 0.04) : Math.round(requests * noise(i + 2, 0.01));
    const allows = Math.max(0, requests - denies - requiresHuman);

    chainLength += allows; // each allowed request appends one event
    const ts = new Date(START.getTime() + i * 60 * 1000).toISOString();

    // Latency distribution centred around 18ms with the usual long tail.
    const p50 = 14 + noise(i + 3, 6);
    const p95 = p50 + 22 + noise(i + 4, 12);
    const p99 = p95 + 35 + noise(i + 5, 30);

    return {
      ts,
      requests,
      allows,
      denies,
      requires_human: requiresHuman,
      audit_chain_length: chainLength,
      latency_ms: {
        p50: Math.round(p50 * 10) / 10,
        p95: Math.round(p95 * 10) / 10,
        p99: Math.round(p99 * 10) / 10,
      },
    };
  }),
};
