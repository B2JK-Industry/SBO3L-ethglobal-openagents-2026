//! SQLite-backed budget tracker (F-2).
//!
//! `BudgetTracker` enforces the four budget scopes defined in
//! `schemas/policy_v1.json` (`per_tx`, `daily`, `monthly`, `per_provider`)
//! against per-bucket spend rows persisted in `sbo3l-storage`. Persistence
//! survives daemon restart and multi-process deployment — the
//! pre-F-2 in-memory `HashMap` lifetime no longer applies.
//!
//! Two operations:
//!
//! - [`BudgetTracker::check`] — read-only; consults
//!   `Storage::budget_spent_cents` for each applicable bucket and returns
//!   the first cap breach as a [`BudgetDeny`]. No writes.
//! - [`BudgetTracker::commit`] — wraps **policy + budget + audit** in a
//!   single rusqlite transaction via `Storage::finalize_decision`. On
//!   error the transaction is rolled back; no partial budget rows, no
//!   audit row without its budget commit.
//!
//! Per-tx is a single-request cap and never accumulates: rows with
//! `scope = 'per_tx'` are not written by this module.

use std::str::FromStr;

use chrono::{DateTime, Datelike, NaiveDate, NaiveTime, TimeZone, Utc};
use rust_decimal::Decimal;
use thiserror::Error;

use sbo3l_core::aprp::PaymentRequest;
use sbo3l_core::audit::SignedAuditEvent;
use sbo3l_core::signer::DevSigner;
use sbo3l_storage::audit_store::NewAuditEvent;
use sbo3l_storage::budget_store::{usd_str_to_cents, BudgetIncrement};
use sbo3l_storage::{Storage, StorageError};

use crate::model::{Budget, BudgetScope, Policy};
use crate::util::same_origin;

#[derive(Debug, Error)]
pub enum BudgetError {
    #[error("budget value '{0}' is not a decimal in whole cents")]
    BadValue(String),
    #[error("storage failure: {0}")]
    Storage(#[from] StorageError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetDeny {
    /// Always `policy.budget_exceeded` per win-backlog standards (F-2).
    pub deny_code: &'static str,
    pub scope: BudgetScope,
    pub scope_key: Option<String>,
    pub cap_usd: Decimal,
    pub spent_usd: Decimal,
    pub requested_usd: Decimal,
}

/// Stateless namespace; F-2 moved all per-bucket spend out of the process
/// memory and into SQLite. The struct is kept for API stability with
/// pre-F-2 callers: every public method is associated, none take `self`.
#[derive(Debug, Default)]
pub struct BudgetTracker;

impl BudgetTracker {
    /// Identity-only constructor — `BudgetTracker` carries no state.
    pub fn new() -> Self {
        Self
    }

    /// Pure read: ask `storage` for current spend on every bucket the
    /// `policy` declares for `request.agent_id`, and surface the first
    /// (request_amount + bucket_spent > cap) breach as a [`BudgetDeny`].
    /// Returns `Ok(None)` when every applicable bucket is under cap.
    pub fn check(
        storage: &Storage,
        policy: &Policy,
        request: &PaymentRequest,
        now: DateTime<Utc>,
    ) -> Result<Option<BudgetDeny>, BudgetError> {
        let amount = parse_decimal(&request.amount.value)?;
        for budget in &policy.budgets {
            if budget.agent_id != request.agent_id {
                continue;
            }
            if !applies_to_request(budget, request, policy) {
                continue;
            }
            let cap = parse_decimal(&budget.cap_usd)?;
            // PerTx is a single-request cap; never persisted, never
            // accumulated, only compared to the request amount alone.
            let (spent, candidate) = if matches!(budget.scope, BudgetScope::PerTx) {
                (Decimal::ZERO, amount)
            } else {
                let key = bucket_key(budget, request, policy, now);
                let scope_str = scope_repr(budget.scope);
                let cents = storage.budget_spent_cents(&request.agent_id, scope_str, &key)?;
                let spent_dec = cents_to_decimal(cents);
                (spent_dec, spent_dec + amount)
            };
            if candidate > cap {
                return Ok(Some(BudgetDeny {
                    deny_code: "policy.budget_exceeded",
                    scope: budget.scope,
                    scope_key: budget.scope_key.clone(),
                    cap_usd: cap,
                    spent_usd: spent,
                    requested_usd: amount,
                }));
            }
        }
        Ok(None)
    }

    /// Atomic combo: persist every budget increment AND append the audit
    /// event in a single transaction. Returns the signed audit event.
    /// If any step fails the whole transaction rolls back and the error
    /// is bubbled — neither budget rows nor the audit row are visible to
    /// any subsequent reader.
    ///
    /// This is the F-2 "wraps policy + budget + audit in single
    /// transaction" acceptance criterion.
    pub fn commit(
        storage: &mut Storage,
        policy: &Policy,
        request: &PaymentRequest,
        now: DateTime<Utc>,
        audit_event: NewAuditEvent,
        audit_signer: &DevSigner,
    ) -> Result<SignedAuditEvent, BudgetError> {
        let increments = build_increments(policy, request, now)?;
        let signed = storage.finalize_decision(&increments, audit_event, audit_signer)?;
        Ok(signed)
    }
}

/// Translate `policy.budgets` rules + the incoming request into the set of
/// `BudgetIncrement`s the SQLite layer should upsert. Returns an empty
/// vector if no budget applies (e.g. policy has no budgets, or only
/// `per_tx` which is never persisted).
fn build_increments(
    policy: &Policy,
    request: &PaymentRequest,
    now: DateTime<Utc>,
) -> Result<Vec<BudgetIncrement>, BudgetError> {
    let amount_cents = usd_str_to_cents(&request.amount.value)
        .ok_or_else(|| BudgetError::BadValue(request.amount.value.clone()))?;
    let mut out = Vec::new();
    for budget in &policy.budgets {
        if budget.agent_id != request.agent_id {
            continue;
        }
        if !applies_to_request(budget, request, policy) {
            continue;
        }
        // Per-tx caps are evaluated in `check`; they don't accumulate, so
        // we never write a row for them.
        if matches!(budget.scope, BudgetScope::PerTx) {
            continue;
        }
        let cap_cents = usd_str_to_cents(&budget.cap_usd)
            .ok_or_else(|| BudgetError::BadValue(budget.cap_usd.clone()))?;
        out.push(BudgetIncrement {
            agent_id: request.agent_id.clone(),
            scope: scope_repr(budget.scope).to_string(),
            scope_key: bucket_key(budget, request, policy, now),
            delta_cents: amount_cents,
            cap_cents,
            reset_at_unix: reset_at_unix(budget.scope, now),
        });
    }
    Ok(out)
}

fn parse_decimal(s: &str) -> Result<Decimal, BudgetError> {
    Decimal::from_str(s).map_err(|_| BudgetError::BadValue(s.to_string()))
}

fn cents_to_decimal(cents: i64) -> Decimal {
    Decimal::new(cents, 2)
}

fn scope_repr(scope: BudgetScope) -> &'static str {
    match scope {
        BudgetScope::PerTx => "per_tx",
        BudgetScope::Daily => "daily",
        BudgetScope::Monthly => "monthly",
        BudgetScope::PerProvider => "per_provider",
    }
}

fn bucket_key(
    budget: &Budget,
    request: &PaymentRequest,
    policy: &Policy,
    now: DateTime<Utc>,
) -> String {
    match budget.scope {
        BudgetScope::PerTx => "tx".to_string(),
        BudgetScope::Daily => now.format("%Y-%m-%d").to_string(),
        BudgetScope::Monthly => format!("{:04}-{:02}", now.year(), now.month()),
        BudgetScope::PerProvider => policy
            .providers
            .iter()
            .find(|p| same_origin(&p.url, &request.provider_url))
            .map(|p| p.id.clone())
            .unwrap_or_else(|| request.provider_url.clone()),
    }
}

/// The next bucket boundary as a unix epoch second. Informational; not
/// enforced by current logic — rollover is implicit in `bucket_key` (a new
/// day's request creates a new row for the new `scope_key`).
fn reset_at_unix(scope: BudgetScope, now: DateTime<Utc>) -> Option<i64> {
    match scope {
        BudgetScope::PerTx => None,
        BudgetScope::Daily => {
            // Midnight UTC after `now`.
            let next = now.date_naive().succ_opt()?;
            let dt = NaiveDate::and_time(&next, NaiveTime::from_hms_opt(0, 0, 0)?);
            Some(Utc.from_utc_datetime(&dt).timestamp())
        }
        BudgetScope::Monthly => {
            // First of next month UTC.
            let (y, m) = if now.month() == 12 {
                (now.year() + 1, 1)
            } else {
                (now.year(), now.month() + 1)
            };
            let next = NaiveDate::from_ymd_opt(y, m, 1)?;
            let dt = NaiveDate::and_time(&next, NaiveTime::from_hms_opt(0, 0, 0)?);
            Some(Utc.from_utc_datetime(&dt).timestamp())
        }
        BudgetScope::PerProvider => None,
    }
}

fn applies_to_request(budget: &Budget, request: &PaymentRequest, policy: &Policy) -> bool {
    match budget.scope {
        BudgetScope::PerTx | BudgetScope::Daily | BudgetScope::Monthly => true,
        BudgetScope::PerProvider => match &budget.scope_key {
            None => true,
            Some(key) => {
                if let Some(p) = policy
                    .providers
                    .iter()
                    .find(|p| same_origin(&p.url, &request.provider_url))
                {
                    &p.id == key
                } else {
                    key == &request.provider_url
                }
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy_default() -> Policy {
        Policy::parse_json(include_str!(
            "../../../test-corpus/policy/reference_low_risk.json"
        ))
        .unwrap()
    }

    fn aprp_golden() -> PaymentRequest {
        let raw = include_str!("../../../test-corpus/aprp/golden_001_minimal.json");
        let v: serde_json::Value = serde_json::from_str(raw).unwrap();
        serde_json::from_value(v).unwrap()
    }

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-04-27T12:00:00Z")
            .unwrap()
            .into()
    }

    fn signer() -> DevSigner {
        DevSigner::from_seed("audit-signer-v1", [11u8; 32])
    }

    fn audit_event_for(req: &PaymentRequest) -> NewAuditEvent {
        NewAuditEvent {
            event_type: "policy_decided".to_string(),
            actor: "policy_engine".to_string(),
            subject_id: format!("pr-{}", req.nonce),
            payload_hash: "00".repeat(32),
            metadata: serde_json::Map::new(),
            policy_version: Some(1),
            policy_hash: Some("00".repeat(32)),
            attestation_ref: None,
            ts: now(),
        }
    }

    fn minimal_policy_with(budgets: Vec<Budget>) -> Policy {
        let mut p = policy_default();
        p.budgets = budgets;
        p
    }

    #[test]
    fn fresh_storage_passes_for_small_request() {
        let s = Storage::open_in_memory().unwrap();
        let p = policy_default();
        let req = aprp_golden();
        assert!(BudgetTracker::check(&s, &p, &req, now()).unwrap().is_none());
    }

    #[test]
    fn per_tx_cap_blocks_oversized_request_without_writing_any_row() {
        let s = Storage::open_in_memory().unwrap();
        let mut p = policy_default();
        for b in &mut p.budgets {
            if matches!(b.scope, BudgetScope::PerTx) {
                b.cap_usd = "0.01".to_string();
            }
        }
        let req = aprp_golden(); // 0.05 USD
        let deny = BudgetTracker::check(&s, &p, &req, now()).unwrap().unwrap();
        assert_eq!(deny.deny_code, "policy.budget_exceeded");
        assert_eq!(deny.scope, BudgetScope::PerTx);
        // Per-tx never persists — table must be empty even after a deny.
        assert_eq!(s.budget_state_count().unwrap(), 0);
    }

    #[test]
    fn daily_cap_accumulates_across_commits_in_storage() {
        let mut s = Storage::open_in_memory().unwrap();
        let p = minimal_policy_with(vec![Budget {
            agent_id: "research-agent-01".to_string(),
            scope: BudgetScope::Daily,
            scope_key: None,
            cap_usd: "0.10".to_string(),
            soft_cap_usd: None,
        }]);
        let req = aprp_golden(); // 0.05 USD

        // 0 + 0.05 ≤ 0.10 — pass.
        assert!(BudgetTracker::check(&s, &p, &req, now()).unwrap().is_none());
        BudgetTracker::commit(&mut s, &p, &req, now(), audit_event_for(&req), &signer()).unwrap();

        // 0.05 + 0.05 = 0.10 ≤ 0.10 — pass.
        assert!(BudgetTracker::check(&s, &p, &req, now()).unwrap().is_none());
        BudgetTracker::commit(&mut s, &p, &req, now(), audit_event_for(&req), &signer()).unwrap();

        // 0.10 + 0.05 > 0.10 — deny.
        let next = BudgetTracker::check(&s, &p, &req, now()).unwrap().unwrap();
        assert_eq!(next.scope, BudgetScope::Daily);
        assert_eq!(next.cap_usd, Decimal::from_str("0.10").unwrap());
        assert_eq!(next.deny_code, "policy.budget_exceeded");
    }

    #[test]
    fn per_provider_bucket_isolates_providers() {
        let mut s = Storage::open_in_memory().unwrap();
        let p = minimal_policy_with(vec![Budget {
            agent_id: "research-agent-01".to_string(),
            scope: BudgetScope::PerProvider,
            scope_key: Some("api.example.com".to_string()),
            cap_usd: "0.10".to_string(),
            soft_cap_usd: None,
        }]);
        let req = aprp_golden();

        assert!(BudgetTracker::check(&s, &p, &req, now()).unwrap().is_none());
        BudgetTracker::commit(&mut s, &p, &req, now(), audit_event_for(&req), &signer()).unwrap();
        BudgetTracker::commit(&mut s, &p, &req, now(), audit_event_for(&req), &signer()).unwrap();
        let next = BudgetTracker::check(&s, &p, &req, now()).unwrap().unwrap();
        assert_eq!(next.scope, BudgetScope::PerProvider);
        assert_eq!(next.scope_key.as_deref(), Some("api.example.com"));
    }

    #[test]
    fn per_tx_does_not_accumulate_in_storage() {
        let mut s = Storage::open_in_memory().unwrap();
        let p = minimal_policy_with(vec![Budget {
            agent_id: "research-agent-01".to_string(),
            scope: BudgetScope::PerTx,
            scope_key: None,
            cap_usd: "0.10".to_string(),
            soft_cap_usd: None,
        }]);
        let req = aprp_golden(); // 0.05 USD
        for _ in 0..50 {
            assert!(BudgetTracker::check(&s, &p, &req, now()).unwrap().is_none());
            BudgetTracker::commit(&mut s, &p, &req, now(), audit_event_for(&req), &signer())
                .unwrap();
        }
        // 50 audit events, but no budget rows — per-tx never persists.
        assert_eq!(s.budget_state_count().unwrap(), 0);
        assert_eq!(s.audit_count().unwrap(), 50);
    }

    #[test]
    fn budget_state_persists_across_storage_reopen() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        let p = minimal_policy_with(vec![Budget {
            agent_id: "research-agent-01".to_string(),
            scope: BudgetScope::Daily,
            scope_key: None,
            cap_usd: "0.10".to_string(),
            soft_cap_usd: None,
        }]);
        let req = aprp_golden();

        {
            let mut s = Storage::open(&path).unwrap();
            BudgetTracker::commit(&mut s, &p, &req, now(), audit_event_for(&req), &signer())
                .unwrap();
        }
        // Drop the in-memory state, reopen the same file. Spent must
        // survive: 0.05 + 0.06 > 0.10 — deny.
        let s = Storage::open(&path).unwrap();
        let mut req2 = req.clone();
        req2.amount.value = "0.06".to_string();
        let deny = BudgetTracker::check(&s, &p, &req2, now()).unwrap().unwrap();
        assert_eq!(deny.deny_code, "policy.budget_exceeded");
        assert_eq!(deny.scope, BudgetScope::Daily);
    }

    #[test]
    fn commit_with_no_applicable_budget_still_appends_audit_event() {
        // A policy with zero applicable budgets must still produce a
        // signed audit event for the decision — the audit chain is
        // unconditional even when there's nothing to charge.
        let mut s = Storage::open_in_memory().unwrap();
        let p = minimal_policy_with(vec![]); // no budgets
        let req = aprp_golden();
        let signed =
            BudgetTracker::commit(&mut s, &p, &req, now(), audit_event_for(&req), &signer())
                .unwrap();
        assert_eq!(signed.event.seq, 1);
        assert_eq!(s.budget_state_count().unwrap(), 0);
        assert_eq!(s.audit_count().unwrap(), 1);
    }
}
