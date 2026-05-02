//! Cross-chain reputation aggregation (T-3-9).
//!
//! Same agent on N chains (per T-3-8 cross-chain identity) → one
//! aggregated reputation score weighted by recency and chain
//! prominence. The pure function lives here; the per-chain ENS
//! fetch (via UniversalResolver from T-4-5) is wired at the
//! application layer to keep `sbo3l-policy` free of circular deps
//! against `sbo3l-identity`.
//!
//! ## Aggregation model
//!
//! Each input is a [`ChainReputationSnapshot`]: a per-chain score
//! plus when it was observed. The aggregator computes:
//!
//! ```text
//! aggregate = round(sum(w_i * r_i * s_i) / sum(w_i * r_i))
//! ```
//!
//! where:
//! - `s_i` is the per-chain raw score (0..=100),
//! - `w_i` is the chain-prominence weight (mainnet 1.0, L2s 0.5–0.8,
//!   testnets 0.2 by default — caller-overridable),
//! - `r_i` is the recency factor (1.0 if the snapshot is fresher
//!   than `recency_window_secs`, linearly decaying to 0.25 over
//!   the window — same shape as the v2 single-chain age weight in
//!   `reputation::age_weight`).
//!
//! Empty snapshot set returns [`Reputation::MAX`] for the same
//! "innocent until proven otherwise" reason as the single-chain
//! aggregator. This is intentional and called out in tests so a
//! reviewer notices the policy choice.
//!
//! ## Why not a single resolver call across chains
//!
//! A native-feeling alternative would be to pack a single
//! `multicall` reading reputation from N chains and aggregate
//! on-chain. That requires either an L2-aware bridge contract or
//! a state-proof verifier per chain, both of which are
//! week-of-effort outside hackathon scope. The pure-function
//! aggregator over per-chain inputs composes with whatever fetch
//! path the caller wires up — Universal Resolver against mainnet
//! ENS, ENSIP-19 reverse resolution against L2 ENS, or the
//! `sbo3l:cross_chain_attestation` text record (T-3-8) — without
//! any module here knowing the difference.

use std::collections::BTreeMap;

use crate::reputation::Reputation;

/// One observation of an agent's reputation on a single chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainReputationSnapshot {
    /// EVM chain id (e.g. `1` mainnet, `10` Optimism, `137` Polygon).
    pub chain_id: u64,
    /// FQDN of the agent on this chain. Carried for the breakdown
    /// report; not load-bearing for the math.
    pub fqdn: String,
    /// Raw reputation score, 0..=100. Caller has already validated
    /// this — values outside the range are clamped at aggregation
    /// time, not rejected, so a misconfigured fetcher doesn't break
    /// every consumer.
    pub score: u8,
    /// Unix-seconds when the snapshot was read. The aggregator
    /// applies the recency factor relative to `now_secs` passed
    /// to [`aggregate_reputation`].
    pub observed_at: u64,
}

/// Caller-tunable parameters for the aggregator. Sensible defaults
/// via [`AggregateReputationParams::default`]; override only what
/// you need to override.
#[derive(Debug, Clone, PartialEq)]
pub struct AggregateReputationParams {
    /// Snapshots older than this contribute at the floor recency
    /// factor (`0.25`). Default 30 days.
    pub recency_window_secs: u64,
    /// Per-chain weight (chain prominence). Sum doesn't have to be
    /// 1.0 — the aggregator normalises internally. Chains absent
    /// from the map fall back to `default_chain_weight`.
    pub chain_weights: BTreeMap<u64, f64>,
    /// Weight for chains not explicitly listed in `chain_weights`.
    /// Default `0.5`. Tunable for ecosystems that want to penalise
    /// unknown chains harder.
    pub default_chain_weight: f64,
}

impl Default for AggregateReputationParams {
    fn default() -> Self {
        let mut weights = BTreeMap::new();
        // Mainnet is the canonical anchor. Anything else is graded.
        weights.insert(1, 1.0); // Ethereum mainnet
        weights.insert(10, 0.8); // Optimism
        weights.insert(8453, 0.8); // Base
        weights.insert(42161, 0.8); // Arbitrum
        weights.insert(137, 0.6); // Polygon (PoS)
        weights.insert(59144, 0.6); // Linea
        weights.insert(11155111, 0.2); // Sepolia (testnet — minimal weight)
        Self {
            recency_window_secs: 30 * 24 * 60 * 60,
            chain_weights: weights,
            default_chain_weight: 0.5,
        }
    }
}

/// Per-chain contribution to the aggregate. Useful for surfacing
/// "why did the score land here?" in operator-facing reports.
#[derive(Debug, Clone, PartialEq)]
pub struct PerChainContribution {
    pub chain_id: u64,
    pub fqdn: String,
    pub raw_score: u8,
    /// Chain weight applied (after `default_chain_weight` fallback).
    pub chain_weight: f64,
    /// Recency factor applied: 1.0 fresh, 0.25 floor at
    /// `recency_window_secs` and beyond.
    pub recency_factor: f64,
    /// Effective contribution to the numerator: `raw_score * weight * recency`.
    pub effective_contribution: f64,
}

/// Aggregate reputation report. Carries the bottom-line score plus
/// the per-chain breakdown so an auditor can re-derive the math.
#[derive(Debug, Clone, PartialEq)]
pub struct AggregateReputationReport {
    pub aggregate_score: u8,
    pub source_count: usize,
    /// Sum of `chain_weight × recency_factor` across all snapshots.
    /// Useful for confidence reporting — high totals mean many
    /// fresh attestations from prominent chains.
    pub total_weight: f64,
    pub per_chain: Vec<PerChainContribution>,
}

/// Aggregate per-chain reputation snapshots into one weighted score.
///
/// Empty input → [`Reputation::MAX`] (100). Same "innocent until
/// proven otherwise" rule the single-chain aggregator applies for
/// fresh agents.
///
/// Identical snapshots → identical reports. The function is pure and
/// composes with any fetch strategy the caller wires up — Universal
/// Resolver, EIP-712 cross-chain attestations, or arbitrary mock
/// fixtures for tests.
pub fn aggregate_reputation(
    snapshots: &[ChainReputationSnapshot],
    now_secs: u64,
    params: &AggregateReputationParams,
) -> AggregateReputationReport {
    if snapshots.is_empty() {
        return AggregateReputationReport {
            aggregate_score: Reputation::MAX.as_u8(),
            source_count: 0,
            total_weight: 0.0,
            per_chain: Vec::new(),
        };
    }

    let mut numerator = 0.0_f64;
    let mut denominator = 0.0_f64;
    let mut per_chain: Vec<PerChainContribution> = Vec::with_capacity(snapshots.len());

    for snap in snapshots {
        let raw = snap.score.min(100);
        let chain_weight = params
            .chain_weights
            .get(&snap.chain_id)
            .copied()
            .unwrap_or(params.default_chain_weight);
        let recency = recency_factor(now_secs, snap.observed_at, params.recency_window_secs);
        let combined = chain_weight * recency;
        let contribution = (raw as f64) * combined;

        numerator += contribution;
        denominator += combined;

        per_chain.push(PerChainContribution {
            chain_id: snap.chain_id,
            fqdn: snap.fqdn.clone(),
            raw_score: raw,
            chain_weight,
            recency_factor: recency,
            effective_contribution: contribution,
        });
    }

    let aggregate = if denominator > 0.0 {
        let v = (numerator / denominator).round();
        v.clamp(0.0, 100.0) as u8
    } else {
        // Every snapshot was effectively-zero-weighted (chain weight 0
        // AND zero-out-of-window recency). Treat as no signal.
        Reputation::MAX.as_u8()
    };

    AggregateReputationReport {
        aggregate_score: aggregate,
        source_count: snapshots.len(),
        total_weight: denominator,
        per_chain,
    }
}

/// Recency factor for a single snapshot. Linear ramp from `1.0`
/// (fresher than 1 day) to `0.25` (older than `window_secs`),
/// constant outside the ramp.
///
/// Mirrors the shape of `reputation::age_weight` used in
/// single-chain v2 scoring so the cross-chain aggregator and the
/// per-chain raw score apply consistent recency policy. If
/// `observed_at` is in the future relative to `now_secs`, the
/// factor is `1.0` (the verifier accepts fresh-clock-skew silently;
/// the alternative of rejecting the snapshot is too brittle for
/// off-chain clock drift).
pub fn recency_factor(now_secs: u64, observed_at: u64, window_secs: u64) -> f64 {
    if window_secs == 0 {
        return 1.0;
    }
    let age = now_secs.saturating_sub(observed_at);
    let one_day = 24 * 60 * 60_u64;
    if age <= one_day {
        1.0
    } else if age >= window_secs {
        0.25
    } else {
        let span = (window_secs - one_day) as f64;
        let pos = (age - one_day) as f64;
        // Linear from 1.0 down to 0.25 over [1d, window_secs].
        1.0 - 0.75 * (pos / span)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(chain_id: u64, fqdn: &str, score: u8, observed_at: u64) -> ChainReputationSnapshot {
        ChainReputationSnapshot {
            chain_id,
            fqdn: fqdn.to_string(),
            score,
            observed_at,
        }
    }

    #[test]
    fn empty_returns_max_reputation() {
        let r = aggregate_reputation(&[], 1_000_000, &AggregateReputationParams::default());
        assert_eq!(r.aggregate_score, 100);
        assert_eq!(r.source_count, 0);
        assert!(r.per_chain.is_empty());
    }

    #[test]
    fn single_mainnet_snapshot_returns_raw_score() {
        let now = 1_000_000;
        let snaps = vec![snap(1, "research-agent.sbo3lagent.eth", 87, now - 60)];
        let r = aggregate_reputation(&snaps, now, &AggregateReputationParams::default());
        assert_eq!(r.aggregate_score, 87);
        assert_eq!(r.source_count, 1);
        assert_eq!(r.per_chain.len(), 1);
        assert_eq!(r.per_chain[0].raw_score, 87);
        assert!((r.per_chain[0].chain_weight - 1.0).abs() < 1e-9);
        assert!((r.per_chain[0].recency_factor - 1.0).abs() < 1e-9);
    }

    #[test]
    fn equal_weight_equal_recency_is_average() {
        let now = 1_000_000;
        let mut params = AggregateReputationParams::default();
        // Force all chains equal weight = 1.0 to test the pure average shape.
        params.chain_weights = BTreeMap::new();
        params.default_chain_weight = 1.0;

        let snaps = vec![
            snap(1, "a", 60, now),
            snap(10, "a", 90, now),
            snap(8453, "a", 80, now),
        ];
        let r = aggregate_reputation(&snaps, now, &params);
        // (60+90+80)/3 = 76.66 → 77
        assert_eq!(r.aggregate_score, 77);
        assert_eq!(r.source_count, 3);
    }

    #[test]
    fn mainnet_outweighs_l2_at_default_weights() {
        let now = 1_000_000;
        let p = AggregateReputationParams::default();
        // Mainnet says 100, Polygon (weight 0.6) says 0. Mainnet
        // weight 1.0 vs Polygon 0.6 → numerator = 100*1 + 0*0.6 = 100,
        // denominator = 1 + 0.6 = 1.6 → 62.5 → rounds to 63.
        let snaps = vec![snap(1, "a", 100, now), snap(137, "a", 0, now)];
        let r = aggregate_reputation(&snaps, now, &p);
        assert_eq!(r.aggregate_score, 63);
    }

    #[test]
    fn unknown_chain_uses_default_weight() {
        let now = 1_000_000;
        let p = AggregateReputationParams::default();
        // Unknown chain id 99999 falls back to default 0.5.
        let snaps = vec![snap(99999, "a", 50, now)];
        let r = aggregate_reputation(&snaps, now, &p);
        assert_eq!(r.aggregate_score, 50);
        assert!((r.per_chain[0].chain_weight - 0.5).abs() < 1e-9);
    }

    /// Use a 2026-era timestamp so 30+ days of subtraction never
    /// underflows on `u64`. (Smaller `now` values like `1_000_000`
    /// can't represent "30 days ago" — the subtraction would go
    /// negative.)
    const NOW_2026: u64 = 1_767_225_600; // 2026-01-01 00:00 UTC

    #[test]
    fn recency_factor_within_one_day_is_one() {
        let now = NOW_2026;
        // Within 1 day → 1.0
        let f = recency_factor(now, now - 12 * 3600, 30 * 86400);
        assert!((f - 1.0).abs() < 1e-9);
    }

    #[test]
    fn recency_factor_at_window_floors_at_quarter() {
        let now = NOW_2026;
        // Exactly window_secs old → 0.25
        let f = recency_factor(now, now - 30 * 86400, 30 * 86400);
        assert!((f - 0.25).abs() < 1e-9);
    }

    #[test]
    fn recency_factor_beyond_window_stays_at_quarter() {
        let now = NOW_2026;
        let f = recency_factor(now, now - 365 * 86400, 30 * 86400);
        assert!((f - 0.25).abs() < 1e-9);
    }

    #[test]
    fn recency_factor_future_observation_is_one() {
        let now = NOW_2026;
        // observed_at > now (clock skew on the source side) → 1.0
        let f = recency_factor(now, now + 60, 30 * 86400);
        assert!((f - 1.0).abs() < 1e-9);
    }

    #[test]
    fn old_snapshot_downweighted() {
        let now = NOW_2026;
        let p = AggregateReputationParams::default();
        // Two mainnet snapshots: one fresh (score 100), one 30+ days old (score 0).
        // Fresh: weight 1.0 × recency 1.0 = 1.0; contribution 100.
        // Old:  weight 1.0 × recency 0.25 = 0.25; contribution 0.
        // numerator = 100; denominator = 1.25 → 80.
        let snaps = vec![
            snap(1, "a", 100, now - 60),
            snap(1, "a", 0, now - 31 * 86400),
        ];
        let r = aggregate_reputation(&snaps, now, &p);
        assert_eq!(r.aggregate_score, 80);
    }

    #[test]
    fn three_chain_synthetic_fleet_aggregates() {
        // A "synthetic 3-chain fleet": same agent on mainnet, Optimism,
        // Polygon. Mainnet says 90 (fresh), Optimism 80 (fresh), Polygon
        // 70 (fresh).
        let now = 2_000_000_000;
        let p = AggregateReputationParams::default();
        let snaps = vec![
            snap(1, "research-agent.sbo3lagent.eth", 90, now - 60),
            snap(10, "research-agent.sbo3lagent.eth", 80, now - 60),
            snap(137, "research-agent.sbo3lagent.eth", 70, now - 60),
        ];
        let r = aggregate_reputation(&snaps, now, &p);
        // numerator = 90*1.0 + 80*0.8 + 70*0.6 = 90 + 64 + 42 = 196
        // denominator = 1.0 + 0.8 + 0.6 = 2.4
        // 196/2.4 = 81.66 → 82
        assert_eq!(r.aggregate_score, 82);
        assert_eq!(r.source_count, 3);
        assert_eq!(r.per_chain.len(), 3);
    }

    #[test]
    fn raw_score_above_100_is_clamped() {
        let now = 1_000_000;
        // Misbehaving fetcher returns 200; aggregator clamps to 100.
        let snaps = vec![snap(1, "a", 200, now)];
        let r = aggregate_reputation(&snaps, now, &AggregateReputationParams::default());
        assert_eq!(r.aggregate_score, 100);
        assert_eq!(r.per_chain[0].raw_score, 100);
    }

    #[test]
    fn report_is_deterministic() {
        let now = 1_000_000_000;
        let p = AggregateReputationParams::default();
        let snaps = vec![snap(1, "a", 90, now - 60), snap(10, "a", 80, now - 86400)];
        let r1 = aggregate_reputation(&snaps, now, &p);
        let r2 = aggregate_reputation(&snaps, now, &p);
        assert_eq!(r1, r2);
    }

    #[test]
    fn total_weight_reflects_snapshot_set() {
        let now = 1_000_000;
        let p = AggregateReputationParams::default();
        let snaps = vec![
            snap(1, "a", 50, now),   // weight 1.0 × recency 1.0 = 1.0
            snap(10, "a", 50, now),  // weight 0.8 × recency 1.0 = 0.8
            snap(137, "a", 50, now), // weight 0.6 × recency 1.0 = 0.6
        ];
        let r = aggregate_reputation(&snaps, now, &p);
        assert!((r.total_weight - 2.4).abs() < 1e-6);
    }

    #[test]
    fn zero_weight_chain_contributes_zero() {
        let now = 1_000_000;
        let mut params = AggregateReputationParams::default();
        params.chain_weights.insert(99999, 0.0);
        // Two snapshots: zero-weight chain says 0, mainnet says 100.
        let snaps = vec![snap(99999, "a", 0, now), snap(1, "a", 100, now)];
        let r = aggregate_reputation(&snaps, now, &params);
        // Numerator = 0*0 + 100*1 = 100; denominator = 0 + 1 = 1 → 100.
        assert_eq!(r.aggregate_score, 100);
    }

    #[test]
    fn all_zero_weight_returns_max_reputation() {
        // Pathological: every chain weighted zero. No signal → 100.
        let now = 1_000_000;
        let mut params = AggregateReputationParams::default();
        params.chain_weights = BTreeMap::new();
        params.default_chain_weight = 0.0;

        let snaps = vec![snap(1, "a", 0, now)];
        let r = aggregate_reputation(&snaps, now, &params);
        assert_eq!(r.aggregate_score, 100);
    }

    #[test]
    fn per_chain_breakdown_preserves_order() {
        let now = 1_000_000;
        let p = AggregateReputationParams::default();
        let snaps = vec![
            snap(8453, "z", 50, now),
            snap(1, "a", 100, now),
            snap(137, "m", 75, now),
        ];
        let r = aggregate_reputation(&snaps, now, &p);
        assert_eq!(r.per_chain[0].chain_id, 8453);
        assert_eq!(r.per_chain[1].chain_id, 1);
        assert_eq!(r.per_chain[2].chain_id, 137);
    }
}
