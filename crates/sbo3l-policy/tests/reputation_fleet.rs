//! Integration test: 5-agent fleet with varied audit histories.
//!
//! Mirrors the T-3-3 fleet (`scripts/fleet-config/agents-5.yaml`) so
//! the reputation publisher can be smoke-tested against the same
//! cardinality the live fleet uses, without needing the live fleet to
//! exist first. Each synthetic agent has a hand-built audit chain
//! whose composition exercises a different scoring branch:
//!
//!   research-agent      — clean (all allows + executor-confirmed)
//!   trading-agent       — mixed (mostly clean, one fresh deny)
//!   swap-agent          — mostly-confirmed (some allows weren't executed)
//!   audit-agent         — long quiet history (no denies, low volume)
//!   coordinator-agent   — bad history (multiple recent denies)
//!
//! Asserts:
//!   * Each agent's score lands in the expected band.
//!   * Scores are deterministic across re-runs.
//!   * Cross-agent ordering is consistent (clean > mostly-clean >
//!     bad).

use std::time::Duration;

use sbo3l_policy::reputation::{compute_reputation_v2, Reputation, ReputationEvent};

fn allow_confirmed(age_days: u64) -> ReputationEvent {
    ReputationEvent {
        decision: "allow".into(),
        executor_confirmed: true,
        age: Duration::from_secs(age_days * 24 * 60 * 60),
    }
}

fn allow_unconfirmed(age_days: u64) -> ReputationEvent {
    ReputationEvent {
        decision: "allow".into(),
        executor_confirmed: false,
        age: Duration::from_secs(age_days * 24 * 60 * 60),
    }
}

fn deny(age_days: u64) -> ReputationEvent {
    ReputationEvent {
        decision: "deny".into(),
        executor_confirmed: false,
        age: Duration::from_secs(age_days * 24 * 60 * 60),
    }
}

fn research_agent_history() -> Vec<ReputationEvent> {
    // 50 clean, executor-confirmed allows over 6 weeks. Long, clean,
    // confirmed. The "good actor" baseline.
    (0..50).map(allow_confirmed).collect()
}

fn trading_agent_history() -> Vec<ReputationEvent> {
    // 40 clean allows + 1 fresh deny (recent budget breach, e.g.).
    let mut events: Vec<_> = (0..40).map(allow_confirmed).collect();
    events.push(deny(0));
    events
}

fn swap_agent_history() -> Vec<ReputationEvent> {
    // 30 allows total, but only 20 were executor-confirmed. The
    // "requested but didn't follow through 33% of the time" pattern.
    let mut events: Vec<_> = (0..20).map(allow_confirmed).collect();
    events.extend((20..30).map(allow_unconfirmed));
    events
}

fn audit_agent_history() -> Vec<ReputationEvent> {
    // 5 clean allows over 6 months. Quiet history, no denies, low
    // volume → no stability bonus. Innocent but new.
    (0..5).map(|i| allow_confirmed(i * 30)).collect()
}

fn coordinator_agent_history() -> Vec<ReputationEvent> {
    // 20 events: 5 clean + 5 unconfirmed allows + 10 fresh denies.
    // The "bad actor" — many recent policy violations.
    let mut events: Vec<_> = (0..5).map(allow_confirmed).collect();
    events.extend((5..10).map(allow_unconfirmed));
    events.extend((0..10).map(deny));
    events
}

#[test]
fn fleet_scores_in_expected_bands() {
    let research = compute_reputation_v2(research_agent_history());
    let trading = compute_reputation_v2(trading_agent_history());
    let swap = compute_reputation_v2(swap_agent_history());
    let audit = compute_reputation_v2(audit_agent_history());
    let coordinator = compute_reputation_v2(coordinator_agent_history());

    println!("research:    {}", research.as_u8());
    println!("trading:     {}", trading.as_u8());
    println!("swap:        {}", swap.as_u8());
    println!("audit:       {}", audit.as_u8());
    println!("coordinator: {}", coordinator.as_u8());

    // Bands. The exact numbers shift if the weighting changes, but
    // the ordering is the canonical contract.
    assert!(
        research.as_u8() >= 90,
        "clean fleet: expected score >= 90, got {}",
        research.as_u8(),
    );
    assert!(
        trading.as_u8() >= 75 && trading.as_u8() < research.as_u8(),
        "mostly-clean: expected [75, {}); got {}",
        research.as_u8(),
        trading.as_u8(),
    );
    assert!(
        swap.as_u8() >= 65 && swap.as_u8() < trading.as_u8(),
        "mostly-confirmed: expected [65, {}); got {}",
        trading.as_u8(),
        swap.as_u8(),
    );
    assert!(
        audit.as_u8() >= 80,
        "quiet clean low-volume: expected >= 80, got {}",
        audit.as_u8(),
    );
    assert!(
        coordinator.as_u8() < Reputation::DEFAULT_REFUSAL_THRESHOLD.as_u8(),
        "bad actor: expected below refusal threshold (60), got {}",
        coordinator.as_u8(),
    );
}

#[test]
fn fleet_scores_are_deterministic() {
    // Run the computation twice; the seeds + ages are fixed in
    // history builders so the score is deterministic.
    for _ in 0..3 {
        let r1 = compute_reputation_v2(research_agent_history()).as_u8();
        let r2 = compute_reputation_v2(research_agent_history()).as_u8();
        assert_eq!(r1, r2);
    }
}

#[test]
fn refusal_threshold_filters_only_coordinator() {
    let scores = [
        (
            "research-agent",
            compute_reputation_v2(research_agent_history()).as_u8(),
        ),
        (
            "trading-agent",
            compute_reputation_v2(trading_agent_history()).as_u8(),
        ),
        (
            "swap-agent",
            compute_reputation_v2(swap_agent_history()).as_u8(),
        ),
        (
            "audit-agent",
            compute_reputation_v2(audit_agent_history()).as_u8(),
        ),
        (
            "coordinator-agent",
            compute_reputation_v2(coordinator_agent_history()).as_u8(),
        ),
    ];
    let threshold = Reputation::DEFAULT_REFUSAL_THRESHOLD.as_u8();
    let refused: Vec<_> = scores
        .iter()
        .filter(|(_, s)| *s < threshold)
        .map(|(name, _)| *name)
        .collect();

    assert_eq!(
        refused,
        vec!["coordinator-agent"],
        "expected only the bad actor to be below threshold {threshold}; got {refused:?}",
    );
}
