//! Cross-agent reputation (T-4-3).
//!
//! Computes a 0-100 reputation score for an agent from its audit
//! chain. Two surfaces:
//!
//! - [`compute_reputation`] — simple allow/deny ratio. Pure function
//!   over an audit-event iterator. Cheap to compute, useful for
//!   `sbo3l doctor`-style offline reports.
//! - [`compute_reputation_v2`] — 4-criteria weighted score covering
//!   clean signed receipts, denials, executor-confirmed receipts,
//!   and an age-weighted recency penalty for fresh denials. Maps to
//!   the same 0-100 range; the wire format on ENS
//!   (`sbo3l:reputation`) doesn't change between the two.
//!
//! Both are pure-function; the publisher
//! (`crates/sbo3l-identity/src/reputation_publisher.rs`, follow-up
//! to this lift) reads `Storage::audit_chain_prefix_through` and
//! emits a `setText(<sbo3l:reputation>, "<score>")` update on each
//! checkpoint creation. The cross-agent attestation hook refuses
//! delegation below [`Reputation::DEFAULT_REFUSAL_THRESHOLD`].
//!
//! ## Lifting from DRAFT (2026-05-01)
//!
//! [`compute_reputation_v2`] + the 5-agent integration-test fixture
//! at `crates/sbo3l-policy/tests/reputation_fleet.rs` complete the
//! pure-compute surface T-4-3 needs. Publisher + CLI subcommand
//! `sbo3l agent reputation <fqdn>` follow once #116 (T-3-1 dry-run)
//! merges and the broadcast slice gets us live agents to read.

use std::time::Duration;

/// One row of an agent's audit chain. The publisher plumbs this
/// from `Storage::audit_chain_prefix_through` rather than reusing
/// `SignedAuditEvent` directly — we don't need signatures or hash
/// linkage to *compute* reputation, just the decisions themselves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReputationRow {
    /// `"allow"` or `"deny"` — taken from `SignedAuditEvent.payload.decision`.
    pub decision: String,
}

/// Richer audit-event shape for [`compute_reputation_v2`]. Captures
/// the signal needed for the 4-criteria scoring without dragging in
/// the full `SignedAuditEvent` type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReputationEvent {
    /// `"allow"` or `"deny"`.
    pub decision: String,
    /// Whether the signed receipt for this decision was carried
    /// through to a sponsor adapter and confirmed (`execution_ref`
    /// landed). False for `deny` rows; true for `allow` rows whose
    /// downstream broadcast succeeded; false for `allow` rows that
    /// stalled or rolled back. Acts as the "did the agent *follow
    /// through*" signal — an agent that gets allow decisions but
    /// never executes them looks suspicious.
    pub executor_confirmed: bool,
    /// Age of the event at scoring time. Recent denials weigh more
    /// than ancient ones (the recency penalty in v2 is bounded so
    /// an old clean run can't be wholly erased by a single fresh
    /// deny).
    pub age: Duration,
}

/// Reputation score range. Pinned to 0..=100 for ENS text-record
/// conventions ("87" reads better than a floating-point ratio in
/// `viem.getEnsText` output).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reputation(u8);

impl Reputation {
    /// Maximum score (100). A fresh agent with no audit history
    /// returns this — "innocent until proven otherwise" matches the
    /// way other reputation systems (GitHub stars, npm downloads)
    /// treat the empty case.
    pub const MAX: Self = Self(100);

    /// Default cross-agent attestation refusal threshold.
    pub const DEFAULT_REFUSAL_THRESHOLD: Self = Self(60);

    pub fn as_u8(self) -> u8 {
        self.0
    }

    /// Render as the string form that goes into the ENS
    /// `sbo3l:reputation` text record.
    pub fn to_text_record(self) -> String {
        self.0.to_string()
    }
}

/// Compute reputation from an iterator of audit decisions.
///
/// Score = round(100 * allow_count / total_count). Empty iterator
/// returns [`Reputation::MAX`] — a fresh agent has no track record
/// yet; we don't punish the empty case.
///
/// Design note: T-4-3's main PR will weight by *recency* (recent
/// denials hurt more than ancient ones) per a Phase 3 amplifier
/// roadmap; the basic ratio here is the floor that the publisher
/// ships first.
pub fn compute_reputation<I>(rows: I) -> Reputation
where
    I: IntoIterator<Item = ReputationRow>,
{
    let mut allow_count: u32 = 0;
    let mut total: u32 = 0;
    for row in rows {
        total = total.saturating_add(1);
        if row.decision == "allow" {
            allow_count = allow_count.saturating_add(1);
        }
    }
    if total == 0 {
        return Reputation::MAX;
    }
    // 100 * allow / total, rounded to nearest. The +50 is the
    // round-half-up bias.
    let score = (100u64 * allow_count as u64 + total as u64 / 2) / total as u64;
    Reputation(score.min(100) as u8)
}

/// 4-criteria reputation, weighted. Compared to the simple ratio:
///
/// - **clean signed receipts** — `allow` decisions with
///   `executor_confirmed = true`. The "did the agent do the right
///   thing AND follow through" signal.
/// - **denials** — `deny` decisions; capped contribution so a single
///   policy denial doesn't tank an otherwise-clean record.
/// - **executor confirmations** — `allow` decisions WITHOUT executor
///   confirmation. Penalised lightly: an agent that requests but
///   doesn't execute looks suspicious but isn't malicious.
/// - **age weight** — recent denials count for more than ancient
///   ones. Linear ramp from 1.0 (fresh, ≤ 1 day old) down to 0.25
///   (older than 30 days). Bounded so an old clean record can't be
///   wholly erased by a single fresh deny.
///
/// The four contributions sum to a 0..=100 score, computed as:
///
/// ```text
/// score = 100 * (60% * clean_ratio
///                + 20% * (1 - weighted_deny_ratio)
///                + 15% * confirm_ratio
///                + 5% * stability_bonus)
/// ```
///
/// where the inner ratios are bounded to `[0, 1]`. Empty input
/// returns [`Reputation::MAX`] for the same "innocent until proven
/// otherwise" reason as [`compute_reputation`].
pub fn compute_reputation_v2<I>(events: I) -> Reputation
where
    I: IntoIterator<Item = ReputationEvent>,
{
    let mut clean_count: u64 = 0; // allow + executor_confirmed
    let mut allow_unconfirmed: u64 = 0; // allow + !executor_confirmed
    let mut deny_weighted: f64 = 0.0; // deny count weighted by recency
    let mut total: u64 = 0;

    for event in events {
        total += 1;
        match event.decision.as_str() {
            "allow" if event.executor_confirmed => clean_count += 1,
            "allow" => allow_unconfirmed += 1,
            "deny" => deny_weighted += age_weight(event.age),
            _ => {} // unknown decisions ignored — don't punish them
        }
    }

    if total == 0 {
        return Reputation::MAX;
    }

    let total_f = total as f64;
    let clean_ratio = (clean_count as f64) / total_f;
    let confirm_ratio = if clean_count + allow_unconfirmed > 0 {
        (clean_count as f64) / ((clean_count + allow_unconfirmed) as f64)
    } else {
        0.0
    };
    let weighted_deny_ratio = (deny_weighted / total_f).clamp(0.0, 1.0);
    // Stability bonus: the more total events the more confident the
    // score; scale linearly to 1.0 at 100+ events.
    let stability_bonus = (total_f / 100.0).clamp(0.0, 1.0);

    let score = 100.0
        * (0.60 * clean_ratio
            + 0.20 * (1.0 - weighted_deny_ratio)
            + 0.15 * confirm_ratio
            + 0.05 * stability_bonus);

    Reputation(score.round().clamp(0.0, 100.0) as u8)
}

/// Age weight in `[0.25, 1.0]`. ≤ 1 day → 1.0 (full weight); 30+
/// days → 0.25 (floor). Linear ramp in between.
fn age_weight(age: Duration) -> f64 {
    let one_day = Duration::from_secs(24 * 60 * 60);
    let thirty_days = Duration::from_secs(30 * 24 * 60 * 60);
    if age <= one_day {
        1.0
    } else if age >= thirty_days {
        0.25
    } else {
        let span = (thirty_days - one_day).as_secs_f64();
        let pos = (age - one_day).as_secs_f64();
        // Linear from 1.0 down to 0.25 over [1d, 30d].
        1.0 - 0.75 * (pos / span)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(decision: &str) -> ReputationRow {
        ReputationRow {
            decision: decision.to_string(),
        }
    }

    #[test]
    fn empty_chain_is_max_reputation() {
        let r = compute_reputation(std::iter::empty());
        assert_eq!(r.as_u8(), 100);
    }

    #[test]
    fn all_allows_is_max_reputation() {
        let r = compute_reputation(vec![row("allow"), row("allow"), row("allow")]);
        assert_eq!(r.as_u8(), 100);
    }

    #[test]
    fn all_denies_is_zero_reputation() {
        let r = compute_reputation(vec![row("deny"), row("deny")]);
        assert_eq!(r.as_u8(), 0);
    }

    #[test]
    fn half_allows_is_fifty() {
        let r = compute_reputation(vec![row("allow"), row("deny")]);
        assert_eq!(r.as_u8(), 50);
    }

    #[test]
    fn ten_allows_three_denies_is_seventy_seven() {
        let mut rows = Vec::new();
        for _ in 0..10 {
            rows.push(row("allow"));
        }
        for _ in 0..3 {
            rows.push(row("deny"));
        }
        let r = compute_reputation(rows);
        // 1000 / 13 = 76.923 → rounds to 77.
        assert_eq!(r.as_u8(), 77);
    }

    #[test]
    fn unknown_decision_counts_as_non_allow() {
        let r = compute_reputation(vec![row("allow"), row("queue"), row("unknown")]);
        // 1 of 3 = 33.33 → 33
        assert_eq!(r.as_u8(), 33);
    }

    #[test]
    fn to_text_record_is_decimal() {
        assert_eq!(Reputation::MAX.to_text_record(), "100");
        assert_eq!(Reputation(0).to_text_record(), "0");
        assert_eq!(Reputation(87).to_text_record(), "87");
    }

    #[test]
    fn refusal_threshold_default_is_sixty() {
        assert_eq!(Reputation::DEFAULT_REFUSAL_THRESHOLD.as_u8(), 60);
    }

    fn ev(decision: &str, executor_confirmed: bool, age_days: u64) -> ReputationEvent {
        ReputationEvent {
            decision: decision.to_string(),
            executor_confirmed,
            age: Duration::from_secs(age_days * 24 * 60 * 60),
        }
    }

    #[test]
    fn v2_empty_chain_is_max_reputation() {
        let r = compute_reputation_v2(std::iter::empty());
        assert_eq!(r.as_u8(), 100);
    }

    #[test]
    fn v2_age_weight_caps_at_one_day() {
        // Same total composition, different ages — older deny weighs less.
        let recent = compute_reputation_v2(vec![ev("allow", true, 0), ev("deny", false, 0)]);
        let ancient = compute_reputation_v2(vec![ev("allow", true, 0), ev("deny", false, 365)]);
        assert!(
            ancient.as_u8() > recent.as_u8(),
            "ancient deny should weigh less than fresh deny: ancient={}, recent={}",
            ancient.as_u8(),
            recent.as_u8(),
        );
    }

    #[test]
    fn v2_executor_confirmation_matters() {
        // 5 allows, all executor-confirmed.
        let confirmed = compute_reputation_v2((0..5).map(|_| ev("allow", true, 0)));
        // 5 allows, none confirmed (agent requested but never followed through).
        let unconfirmed = compute_reputation_v2((0..5).map(|_| ev("allow", false, 0)));
        assert!(
            confirmed.as_u8() > unconfirmed.as_u8(),
            "confirmed allows should score higher: confirmed={}, unconfirmed={}",
            confirmed.as_u8(),
            unconfirmed.as_u8(),
        );
    }

    #[test]
    fn v2_all_clean_high_volume_approaches_max() {
        let events: Vec<_> = (0..120).map(|_| ev("allow", true, 1)).collect();
        let r = compute_reputation_v2(events);
        // 60% clean + 20% (1 - 0 weighted deny) + 15% confirm + 5%
        // stability bonus = 100% at >= 100 events.
        assert_eq!(r.as_u8(), 100);
    }

    #[test]
    fn v2_low_volume_gets_partial_stability_bonus() {
        // 10 events all clean and confirmed — stability_bonus = 0.10.
        let events: Vec<_> = (0..10).map(|_| ev("allow", true, 0)).collect();
        let r = compute_reputation_v2(events);
        // 60 * 1.0 + 20 * 1.0 + 15 * 1.0 + 5 * 0.10 = 95.5 → 96
        assert_eq!(r.as_u8(), 96);
    }

    #[test]
    fn v2_unknown_decisions_are_ignored() {
        // Unknown decisions don't count for or against.
        let r = compute_reputation_v2(vec![
            ev("allow", true, 0),
            ev("queue", false, 0),
            ev("rate_limited", false, 0),
        ]);
        // total=3, clean=1 (1/3 ≈ 0.333), no denies, confirm=1/1=1.0,
        // stability_bonus = 0.03.
        // 60 * 0.333 + 20 * 1.0 + 15 * 1.0 + 5 * 0.03 = 55.15 → 55
        assert_eq!(r.as_u8(), 55);
    }

    #[test]
    fn age_weight_floor_at_thirty_days() {
        let w = age_weight(Duration::from_secs(30 * 24 * 60 * 60));
        assert!((w - 0.25).abs() < 1e-9, "30-day weight = 0.25, got {w}");
    }

    #[test]
    fn age_weight_ceiling_at_one_day() {
        let w = age_weight(Duration::from_secs(24 * 60 * 60));
        assert!((w - 1.0).abs() < 1e-9, "1-day weight = 1.0, got {w}");
    }

    #[test]
    fn age_weight_linear_in_between() {
        // ~15.5 days = midpoint of [1d, 30d] → weight ≈ 0.625.
        let w = age_weight(Duration::from_secs((24 * 60 * 60) * 16)); // 16 days
        let mid = (1.0 + 0.25) / 2.0;
        assert!(
            (w - mid).abs() < 0.05,
            "16-day weight near midpoint: expected ~{mid}, got {w}"
        );
    }
}
