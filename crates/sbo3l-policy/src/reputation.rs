//! Cross-agent reputation — DRAFT (T-4-3).
//!
//! Computes a 0-100 reputation score for an agent from its audit
//! chain success rate (allowed decisions / total decisions). T-4-3
//! main PR adds:
//!
//! 1. [`compute_reputation`] (this file) — pure function over an
//!    audit-event iterator.
//! 2. A publisher in `crates/sbo3l-identity/src/reputation_publisher.rs`
//!    that reads the audit chain from SQLite, computes the score, and
//!    emits a `setText(<sbo3l:reputation>, "<score>")` calldata
//!    update on each checkpoint creation.
//! 3. A consumer hook in cross-agent attestation that refuses
//!    delegation if the target's reputation is below a configurable
//!    threshold (default 60/100 — empirically tuned during Phase 3
//!    amplifier work).
//!
//! **Status: DRAFT.** Depends on T-3-3 (5+ named agents on Sepolia
//! with full sbo3l:* records) so the publisher has agents to update.
//! Until then, this is a deterministic pure function with unit tests
//! — useful on its own for offline reputation computation in
//! `sbo3l doctor` reports.

/// One row of an agent's audit chain. T-4-3's main PR plumbs this
/// from `Storage::audit_chain_prefix_through` rather than reusing
/// `SignedAuditEvent` directly — we don't need signatures or hash
/// linkage to *compute* reputation, just the decisions themselves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReputationRow {
    /// `"allow"` or `"deny"` — taken from `SignedAuditEvent.payload.decision`.
    pub decision: String,
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
}
