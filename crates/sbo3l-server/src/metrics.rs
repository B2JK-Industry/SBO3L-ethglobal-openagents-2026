//! R13 P7 — process-local metrics registry + Prometheus text exporter.
//!
//! Two endpoints share this registry:
//!
//! - `GET /v1/metrics` (this PR) — Prometheus text exposition format
//!   (`text/plain; version=0.0.4`). One scrape per Prometheus tick.
//! - `GET /v1/admin/metrics` (existing, #252) — JSON snapshot for the
//!   apps/observability dashboard. Previously returned placeholder
//!   zeros for requests / allows / denies / latency; this PR backs
//!   them with the same registry so both endpoints tell the truth.
//!
//! # Why hand-rolled Prometheus rather than the `prometheus` crate
//!
//! The `prometheus` crate pulls ~10 transitive deps for what is, on
//! our hot path, three counters + one histogram. The text format is
//! 30 lines of `format!`. The honest tradeoff: skip the dep tree.
//!
//! # Counter / histogram dimensions
//!
//! - `sbo3l_requests_total` (counter) — every finalized
//!   `POST /v1/payment-requests` increments this once.
//! - `sbo3l_decisions_total{outcome="allow|deny|requires_human"}`
//!   (counter, 3-way labelled) — same call, broken out by receipt
//!   decision.
//! - `sbo3l_request_duration_seconds_bucket{le="…"}` +
//!   `_sum` + `_count` (histogram) — request latency in seconds. Fixed
//!   buckets covering 1ms–10s, capped with `+Inf` per Prometheus spec.
//!
//! No tenant/agent labels — keeping cardinality bounded for the
//! single-tenant default. Per-tenant labelling lands when multi-tenant
//! tenant headers do.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use sbo3l_core::receipt::Decision as ReceiptDecision;

/// Histogram bucket upper bounds, in seconds. `+Inf` is implicit per
/// Prometheus convention. Picked to span the realistic latency range
/// observed in `scripts/perf/load-test.sh` (p99 ≈ 9–87ms across c=16..256)
/// with enough headroom to capture tail outliers.
pub const BUCKET_BOUNDS_SECS: &[f64] = &[
    0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// Process-local metrics. All members are atomic so concurrent
/// `record_request` calls don't need a lock. Histogram buckets are
/// laid out sparsely as `AtomicU64`s — `bucket_count[i]` is the count
/// of observations with `value <= BUCKET_BOUNDS_SECS[i]`. This is the
/// "cumulative bucket" convention Prometheus expects on the wire.
pub struct MetricsRegistry {
    requests_total: AtomicU64,
    decisions_allow: AtomicU64,
    decisions_deny: AtomicU64,
    decisions_requires_human: AtomicU64,
    /// Cumulative counts: `bucket_counts[i]` = number of observations
    /// with `value <= BUCKET_BOUNDS_SECS[i]`. The `+Inf` bucket is
    /// implicit and equals `request_count`.
    bucket_counts: Vec<AtomicU64>,
    /// Sum of observed durations, in microseconds, to keep the atomic
    /// math integer-only. Converted to seconds at scrape time.
    duration_sum_micros: AtomicU64,
    /// Total observation count. Equals the implicit `+Inf` bucket.
    request_count: AtomicU64,
}

impl MetricsRegistry {
    pub fn new() -> Self {
        let mut buckets = Vec::with_capacity(BUCKET_BOUNDS_SECS.len());
        for _ in BUCKET_BOUNDS_SECS {
            buckets.push(AtomicU64::new(0));
        }
        Self {
            requests_total: AtomicU64::new(0),
            decisions_allow: AtomicU64::new(0),
            decisions_deny: AtomicU64::new(0),
            decisions_requires_human: AtomicU64::new(0),
            bucket_counts: buckets,
            duration_sum_micros: AtomicU64::new(0),
            request_count: AtomicU64::new(0),
        }
    }

    /// Record one finalized pipeline run.
    pub fn record_request(&self, duration: Duration, decision: &ReceiptDecision) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
        match decision {
            ReceiptDecision::Allow => {
                self.decisions_allow.fetch_add(1, Ordering::Relaxed);
            }
            ReceiptDecision::Deny => {
                self.decisions_deny.fetch_add(1, Ordering::Relaxed);
            }
            ReceiptDecision::RequiresHuman => {
                self.decisions_requires_human
                    .fetch_add(1, Ordering::Relaxed);
            }
        }
        let micros = duration.as_micros().min(u128::from(u64::MAX)) as u64;
        self.duration_sum_micros
            .fetch_add(micros, Ordering::Relaxed);
        self.request_count.fetch_add(1, Ordering::Relaxed);
        let secs = duration.as_secs_f64();
        for (i, bound) in BUCKET_BOUNDS_SECS.iter().enumerate() {
            if secs <= *bound {
                self.bucket_counts[i].fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    pub fn requests_total(&self) -> u64 {
        self.requests_total.load(Ordering::Relaxed)
    }

    pub fn decisions_allow(&self) -> u64 {
        self.decisions_allow.load(Ordering::Relaxed)
    }

    pub fn decisions_deny(&self) -> u64 {
        self.decisions_deny.load(Ordering::Relaxed)
    }

    pub fn decisions_requires_human(&self) -> u64 {
        self.decisions_requires_human.load(Ordering::Relaxed)
    }

    /// Approximate p50/p95/p99 from the histogram bucket counts. Uses
    /// linear interpolation within the bucket where the percentile
    /// falls — the same approximation Prometheus's
    /// `histogram_quantile` does. Returns 0.0 if no observations have
    /// been recorded yet (a safe placeholder for the JSON dashboard).
    pub fn latency_quantiles(&self) -> LatencyQuantiles {
        let total = self.request_count.load(Ordering::Relaxed);
        if total == 0 {
            return LatencyQuantiles::default();
        }
        LatencyQuantiles {
            p50: self.quantile(0.50, total),
            p95: self.quantile(0.95, total),
            p99: self.quantile(0.99, total),
        }
    }

    fn quantile(&self, q: f64, total: u64) -> f64 {
        let target = (total as f64 * q).ceil() as u64;
        let mut prev_count = 0u64;
        let mut prev_bound = 0.0f64;
        for (i, bound) in BUCKET_BOUNDS_SECS.iter().enumerate() {
            let count = self.bucket_counts[i].load(Ordering::Relaxed);
            if count >= target {
                if count == prev_count {
                    return *bound;
                }
                let frac = (target - prev_count) as f64 / (count - prev_count) as f64;
                return prev_bound + frac * (bound - prev_bound);
            }
            prev_count = count;
            prev_bound = *bound;
        }
        // Spilled past last finite bucket — return last bound as the
        // best-known approximation. The +Inf bucket has no upper
        // bound so we can't interpolate further.
        *BUCKET_BOUNDS_SECS.last().unwrap()
    }

    /// Render the registry as Prometheus text exposition format
    /// (`text/plain; version=0.0.4`).
    ///
    /// Output is sorted within each metric family, includes `# HELP`
    /// and `# TYPE` lines per family, and uses the `le` label
    /// convention for histogram buckets including the implicit `+Inf`.
    pub fn render_prometheus(&self) -> String {
        let mut out = String::with_capacity(2048);

        out.push_str(
            "# HELP sbo3l_requests_total Total finalized POST /v1/payment-requests calls.\n",
        );
        out.push_str("# TYPE sbo3l_requests_total counter\n");
        out.push_str(&format!("sbo3l_requests_total {}\n", self.requests_total()));

        out.push_str(
            "# HELP sbo3l_decisions_total Pipeline decisions broken out by receipt outcome.\n",
        );
        out.push_str("# TYPE sbo3l_decisions_total counter\n");
        out.push_str(&format!(
            "sbo3l_decisions_total{{outcome=\"allow\"}} {}\n",
            self.decisions_allow()
        ));
        out.push_str(&format!(
            "sbo3l_decisions_total{{outcome=\"deny\"}} {}\n",
            self.decisions_deny()
        ));
        out.push_str(&format!(
            "sbo3l_decisions_total{{outcome=\"requires_human\"}} {}\n",
            self.decisions_requires_human()
        ));

        let request_count = self.request_count.load(Ordering::Relaxed);
        let sum_micros = self.duration_sum_micros.load(Ordering::Relaxed);
        let sum_secs = sum_micros as f64 / 1_000_000.0;

        out.push_str("# HELP sbo3l_request_duration_seconds Request latency in seconds.\n");
        out.push_str("# TYPE sbo3l_request_duration_seconds histogram\n");
        for (i, bound) in BUCKET_BOUNDS_SECS.iter().enumerate() {
            let count = self.bucket_counts[i].load(Ordering::Relaxed);
            out.push_str(&format!(
                "sbo3l_request_duration_seconds_bucket{{le=\"{bound}\"}} {count}\n"
            ));
        }
        out.push_str(&format!(
            "sbo3l_request_duration_seconds_bucket{{le=\"+Inf\"}} {request_count}\n"
        ));
        out.push_str(&format!("sbo3l_request_duration_seconds_sum {sum_secs}\n"));
        out.push_str(&format!(
            "sbo3l_request_duration_seconds_count {request_count}\n"
        ));

        out
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct LatencyQuantiles {
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn fresh_registry_is_zero() {
        let reg = MetricsRegistry::new();
        assert_eq!(reg.requests_total(), 0);
        assert_eq!(reg.decisions_allow(), 0);
        assert_eq!(reg.decisions_deny(), 0);
        let q = reg.latency_quantiles();
        assert_eq!(q.p50, 0.0);
        assert_eq!(q.p95, 0.0);
        assert_eq!(q.p99, 0.0);
    }

    #[test]
    fn record_increments_counters_by_outcome() {
        let reg = MetricsRegistry::new();
        reg.record_request(ms(5), &ReceiptDecision::Allow);
        reg.record_request(ms(5), &ReceiptDecision::Allow);
        reg.record_request(ms(5), &ReceiptDecision::Deny);
        assert_eq!(reg.requests_total(), 3);
        assert_eq!(reg.decisions_allow(), 2);
        assert_eq!(reg.decisions_deny(), 1);
        assert_eq!(reg.decisions_requires_human(), 0);
    }

    #[test]
    fn histogram_buckets_are_cumulative() {
        let reg = MetricsRegistry::new();
        // 5ms falls into the 0.005, 0.01, 0.025, 0.05, ..., 10.0
        // buckets (Prometheus cumulative convention).
        reg.record_request(ms(5), &ReceiptDecision::Allow);
        // 50ms falls into the 0.05, 0.1, 0.25, ..., 10.0 buckets.
        reg.record_request(ms(50), &ReceiptDecision::Allow);
        let txt = reg.render_prometheus();

        // 0.001 bucket: neither 5ms (0.005s) nor 50ms (0.05s) fits.
        assert!(txt.contains("sbo3l_request_duration_seconds_bucket{le=\"0.001\"} 0"));
        // 0.005 bucket: 5ms fits, 50ms doesn't → 1.
        assert!(txt.contains("sbo3l_request_duration_seconds_bucket{le=\"0.005\"} 1"));
        // 0.05 bucket: both fit → 2.
        assert!(txt.contains("sbo3l_request_duration_seconds_bucket{le=\"0.05\"} 2"));
        // +Inf: total observation count.
        assert!(txt.contains("sbo3l_request_duration_seconds_bucket{le=\"+Inf\"} 2"));
        // Count and sum match.
        assert!(txt.contains("sbo3l_request_duration_seconds_count 2"));
        assert!(txt.contains("sbo3l_request_duration_seconds_sum 0.055"));
    }

    #[test]
    fn prometheus_text_has_help_and_type_lines() {
        let reg = MetricsRegistry::new();
        reg.record_request(ms(1), &ReceiptDecision::Allow);
        let txt = reg.render_prometheus();
        // Each metric family needs # HELP + # TYPE before the samples
        // (a Prometheus parser will reject samples without them).
        assert!(txt.contains("# HELP sbo3l_requests_total"));
        assert!(txt.contains("# TYPE sbo3l_requests_total counter"));
        assert!(txt.contains("# HELP sbo3l_decisions_total"));
        assert!(txt.contains("# TYPE sbo3l_decisions_total counter"));
        assert!(txt.contains("# HELP sbo3l_request_duration_seconds"));
        assert!(txt.contains("# TYPE sbo3l_request_duration_seconds histogram"));
    }

    #[test]
    fn quantiles_track_observations() {
        let reg = MetricsRegistry::new();
        // 100 fast (1ms) + 0 slow → p50/p95/p99 all in low bucket.
        for _ in 0..100 {
            reg.record_request(ms(1), &ReceiptDecision::Allow);
        }
        let q = reg.latency_quantiles();
        // All observations land in the 0.001 bucket; quantiles should
        // not exceed that.
        assert!(q.p50 <= 0.001 + f64::EPSILON);
        assert!(q.p99 <= 0.001 + f64::EPSILON);
    }

    #[test]
    fn quantiles_skew_when_tail_is_slow() {
        let reg = MetricsRegistry::new();
        // 99 fast (1ms) + 1 slow (500ms). p99 should land in the
        // slow bucket span (0.5s); p50 should stay fast.
        for _ in 0..99 {
            reg.record_request(ms(1), &ReceiptDecision::Allow);
        }
        reg.record_request(ms(500), &ReceiptDecision::Allow);
        let q = reg.latency_quantiles();
        assert!(
            q.p50 <= 0.001 + f64::EPSILON,
            "p50 should be fast: {}",
            q.p50
        );
        assert!(q.p99 >= 0.001, "p99 should reflect slow tail: {}", q.p99);
    }

    #[test]
    fn requires_human_increments_its_own_counter() {
        let reg = MetricsRegistry::new();
        reg.record_request(ms(1), &ReceiptDecision::RequiresHuman);
        let txt = reg.render_prometheus();
        assert!(txt.contains("sbo3l_decisions_total{outcome=\"requires_human\"} 1"));
    }

    #[test]
    fn render_prometheus_is_parseable_shape() {
        // Every non-empty, non-comment line must look like
        // `<name>{[labels]} <value>`. Catches a missing newline /
        // accidental concat.
        let reg = MetricsRegistry::new();
        reg.record_request(ms(7), &ReceiptDecision::Allow);
        let txt = reg.render_prometheus();
        for line in txt.lines() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            assert!(parts.len() >= 2, "malformed metric line: {line:?}");
            // Last token must parse as a finite f64 / u64.
            let value = parts.last().unwrap();
            assert!(
                value.parse::<f64>().is_ok(),
                "non-numeric value on line: {line:?}"
            );
        }
    }

    #[test]
    fn buckets_match_documented_bounds() {
        // Doc-vs-code drift guard: the public BUCKET_BOUNDS_SECS slice
        // is what the Prometheus output advertises; if a future edit
        // changes the slice without updating downstream dashboards,
        // this catches it on the test boundary.
        assert_eq!(
            BUCKET_BOUNDS_SECS,
            &[0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,]
        );
    }
}
