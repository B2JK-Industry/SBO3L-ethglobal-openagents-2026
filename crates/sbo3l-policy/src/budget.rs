//! In-memory budget tracker.
//!
//! Implements `per_tx`, `daily`, `monthly` and `per_provider` budget scopes
//! defined in `schemas/policy_v1.json`. Persistence is the responsibility of
//! `sbo3l-storage`; this module only enforces caps against spend the caller
//! reports as committed.

use std::collections::HashMap;
use std::str::FromStr;

use chrono::{DateTime, Datelike, Utc};
use rust_decimal::Decimal;
use thiserror::Error;

use sbo3l_core::aprp::PaymentRequest;

use crate::model::{Budget, BudgetScope, Policy};
use crate::util::same_origin;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BudgetError {
    #[error("budget value '{0}' is not a decimal")]
    BadValue(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetDeny {
    pub deny_code: &'static str,
    pub scope: BudgetScope,
    pub scope_key: Option<String>,
    pub cap_usd: Decimal,
    pub spent_usd: Decimal,
    pub requested_usd: Decimal,
}

#[derive(Debug, Default)]
pub struct BudgetTracker {
    // (agent_id, scope, bucket_key) -> committed spend.
    buckets: HashMap<(String, BudgetScope, String), Decimal>,
}

impl BudgetTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn check(
        &self,
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
            // PerTx is a single-request cap and never accumulates.
            let (spent, candidate) = if matches!(budget.scope, BudgetScope::PerTx) {
                (Decimal::ZERO, amount)
            } else {
                let key = bucket_key(budget, request, policy, now);
                let s = self
                    .buckets
                    .get(&(request.agent_id.clone(), budget.scope, key))
                    .copied()
                    .unwrap_or(Decimal::ZERO);
                (s, s + amount)
            };
            if candidate > cap {
                return Ok(Some(BudgetDeny {
                    deny_code: "budget.hard_cap_exceeded",
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

    pub fn commit(
        &mut self,
        policy: &Policy,
        request: &PaymentRequest,
        now: DateTime<Utc>,
    ) -> Result<(), BudgetError> {
        let amount = parse_decimal(&request.amount.value)?;
        for budget in &policy.budgets {
            if budget.agent_id != request.agent_id {
                continue;
            }
            if !applies_to_request(budget, request, policy) {
                continue;
            }
            // PerTx is a single-request cap and is never persisted across commits.
            if matches!(budget.scope, BudgetScope::PerTx) {
                continue;
            }
            let key = bucket_key(budget, request, policy, now);
            let entry = self
                .buckets
                .entry((request.agent_id.clone(), budget.scope, key))
                .or_insert(Decimal::ZERO);
            *entry += amount;
        }
        Ok(())
    }
}

fn parse_decimal(s: &str) -> Result<Decimal, BudgetError> {
    Decimal::from_str(s).map_err(|_| BudgetError::BadValue(s.to_string()))
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
        BudgetScope::PerProvider => {
            // Bucket by the policy.providers entry whose URL matches the request,
            // or by the raw URL if no policy entry exists.
            let pid = policy
                .providers
                .iter()
                .find(|p| same_origin(&p.url, &request.provider_url))
                .map(|p| p.id.clone())
                .unwrap_or_else(|| request.provider_url.clone());
            pid
        }
    }
}

fn applies_to_request(budget: &Budget, request: &PaymentRequest, policy: &Policy) -> bool {
    match budget.scope {
        BudgetScope::PerTx | BudgetScope::Daily | BudgetScope::Monthly => true,
        BudgetScope::PerProvider => match &budget.scope_key {
            None => true,
            Some(key) => {
                // scope_key may be the provider id; match by id or url.
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

    fn policy() -> Policy {
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

    #[test]
    fn fresh_tracker_passes_for_small_request() {
        let p = policy();
        let req = aprp_golden();
        let t = BudgetTracker::new();
        assert!(t.check(&p, &req, now()).unwrap().is_none());
    }

    #[test]
    fn per_tx_cap_blocks_oversized_request() {
        let mut p = policy();
        // tighten per_tx cap to 0.01 USD
        for b in &mut p.budgets {
            if matches!(b.scope, BudgetScope::PerTx) {
                b.cap_usd = "0.01".to_string();
            }
        }
        let req = aprp_golden();
        let t = BudgetTracker::new();
        let deny = t.check(&p, &req, now()).unwrap().unwrap();
        assert_eq!(deny.deny_code, "budget.hard_cap_exceeded");
        assert_eq!(deny.scope, BudgetScope::PerTx);
    }

    fn minimal_policy_with(budgets: Vec<Budget>) -> Policy {
        let mut p = policy();
        p.budgets = budgets;
        p
    }

    #[test]
    fn daily_cap_accumulates_across_commits() {
        let p = minimal_policy_with(vec![Budget {
            agent_id: "research-agent-01".to_string(),
            scope: BudgetScope::Daily,
            scope_key: None,
            cap_usd: "0.10".to_string(),
            soft_cap_usd: None,
        }]);
        let req = aprp_golden(); // 0.05 USD

        let mut t = BudgetTracker::new();
        assert!(t.check(&p, &req, now()).unwrap().is_none()); // 0 + 0.05 ≤ 0.10
        t.commit(&p, &req, now()).unwrap(); // 0.05 spent
        assert!(t.check(&p, &req, now()).unwrap().is_none()); // 0.05 + 0.05 = 0.10 ≤ 0.10
        t.commit(&p, &req, now()).unwrap(); // 0.10 spent
        let next = t.check(&p, &req, now()).unwrap().unwrap(); // 0.10 + 0.05 > 0.10
        assert_eq!(next.scope, BudgetScope::Daily);
        assert_eq!(next.cap_usd, Decimal::from_str("0.10").unwrap());
    }

    #[test]
    fn per_provider_bucket_isolates_providers() {
        let p = minimal_policy_with(vec![Budget {
            agent_id: "research-agent-01".to_string(),
            scope: BudgetScope::PerProvider,
            scope_key: Some("api.example.com".to_string()),
            cap_usd: "0.10".to_string(),
            soft_cap_usd: None,
        }]);
        let req = aprp_golden(); // provider api.example.com, 0.05

        let mut t = BudgetTracker::new();
        assert!(t.check(&p, &req, now()).unwrap().is_none());
        t.commit(&p, &req, now()).unwrap();
        t.commit(&p, &req, now()).unwrap(); // 0.10 spent
        let next = t.check(&p, &req, now()).unwrap().unwrap();
        assert_eq!(next.scope, BudgetScope::PerProvider);
        assert_eq!(next.scope_key.as_deref(), Some("api.example.com"));
    }

    #[test]
    fn per_tx_does_not_accumulate() {
        let p = minimal_policy_with(vec![Budget {
            agent_id: "research-agent-01".to_string(),
            scope: BudgetScope::PerTx,
            scope_key: None,
            cap_usd: "0.10".to_string(),
            soft_cap_usd: None,
        }]);
        let req = aprp_golden(); // 0.05
        let mut t = BudgetTracker::new();
        for _ in 0..1000 {
            assert!(t.check(&p, &req, now()).unwrap().is_none());
            t.commit(&p, &req, now()).unwrap();
        }
    }
}
