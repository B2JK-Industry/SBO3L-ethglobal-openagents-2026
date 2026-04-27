//! Policy decision engine.
//!
//! Inputs:
//!   * a parsed `Policy`,
//!   * an APRP `PaymentRequest` (already schema-validated),
//!   * an optional emergency override.
//!
//! Output:
//!   * `Decision::Allow / Deny / RequiresHuman`,
//!   * matched rule id,
//!   * deny code if denied.
//!
//! Budget evaluation is intentionally separate (see `crate::budget`).

use serde::{Deserialize, Serialize};
use serde_json::json;

use mandate_core::aprp::{Destination, PaymentRequest};

use crate::expr;
use crate::model::{AgentStatus, Policy, ProviderStatus, RecipientStatus, Rule, RuleEffect};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Allow,
    Deny,
    RequiresHuman,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub decision: Decision,
    pub matched_rule_id: Option<String>,
    pub deny_code: Option<String>,
    pub policy_hash: String,
}

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("rule {rule_id} expression: {source}")]
    Expression {
        rule_id: String,
        #[source]
        source: expr::ExprError,
    },
    #[error("rule {rule_id} effect={effect:?} requires deny_code")]
    DenyCodeMissing { rule_id: String, effect: RuleEffect },
    #[error("policy hash: {0}")]
    Hash(String),
}

/// Evaluate the given policy against an APRP payment request.
pub fn decide(policy: &Policy, request: &PaymentRequest) -> Result<Outcome, EngineError> {
    let context = build_context(policy, request);
    let policy_hash = policy
        .canonical_hash()
        .map_err(|e| EngineError::Hash(e.to_string()))?;

    // Fail-closed agent gate. Runs *before* any rule evaluation so a permissive
    // allow rule can never fire for an agent that is unknown, paused or revoked.
    if let Some(early) = agent_gate(policy, request, &policy_hash) {
        return Ok(early);
    }

    for rule in &policy.rules {
        let matched =
            expr::evaluate_bool(&rule.when, &context).map_err(|e| EngineError::Expression {
                rule_id: rule.id.clone(),
                source: e,
            })?;
        if !matched {
            continue;
        }
        return finalise(rule, policy_hash);
    }

    let decision = match policy.default_decision {
        crate::model::DefaultDecision::Deny => Decision::Deny,
        crate::model::DefaultDecision::RequiresHuman => Decision::RequiresHuman,
    };
    Ok(Outcome {
        decision,
        matched_rule_id: None,
        deny_code: None,
        policy_hash,
    })
}

/// Pre-rule fail-closed gate on agent identity / status / emergency pause list.
///
/// Returns `Some(deny)` when the request must be rejected before any rule runs:
///
/// * agent_id is not registered in `policy.agents` → `auth.agent_not_found`
/// * agent status is `paused` → `emergency.agent_paused`
/// * agent status is `revoked` → `auth.agent_revoked`
/// * agent_id appears in `policy.emergency.paused_agents` → `emergency.agent_paused`
///
/// `None` means the agent is known + active and rule evaluation should proceed.
fn agent_gate(policy: &Policy, request: &PaymentRequest, policy_hash: &str) -> Option<Outcome> {
    let deny = |code: &str| {
        Some(Outcome {
            decision: Decision::Deny,
            matched_rule_id: None,
            deny_code: Some(code.to_string()),
            policy_hash: policy_hash.to_string(),
        })
    };
    let agent = policy
        .agents
        .iter()
        .find(|a| a.agent_id == request.agent_id);
    let Some(agent) = agent else {
        return deny("auth.agent_not_found");
    };
    match agent.status {
        AgentStatus::Active => {}
        AgentStatus::Paused => return deny("emergency.agent_paused"),
        AgentStatus::Revoked => return deny("auth.agent_revoked"),
    }
    if policy
        .emergency
        .paused_agents
        .iter()
        .any(|a| a == &request.agent_id)
    {
        return deny("emergency.agent_paused");
    }
    None
}

fn finalise(rule: &Rule, policy_hash: String) -> Result<Outcome, EngineError> {
    let decision = match rule.effect {
        RuleEffect::Allow => Decision::Allow,
        RuleEffect::Deny => Decision::Deny,
        RuleEffect::RequiresHuman => Decision::RequiresHuman,
    };
    if matches!(rule.effect, RuleEffect::Deny) && rule.deny_code.is_none() {
        return Err(EngineError::DenyCodeMissing {
            rule_id: rule.id.clone(),
            effect: rule.effect,
        });
    }
    Ok(Outcome {
        decision,
        matched_rule_id: Some(rule.id.clone()),
        deny_code: rule.deny_code.clone(),
        policy_hash,
    })
}

fn build_context(policy: &Policy, request: &PaymentRequest) -> serde_json::Value {
    let amount_usd = request.amount.value.parse::<f64>().unwrap_or(f64::INFINITY);

    let provider = match policy
        .providers
        .iter()
        .find(|p| same_origin(&p.url, &request.provider_url))
    {
        Some(p) => json!({
            "id": p.id,
            "url": p.url,
            "trusted": matches!(p.status, ProviderStatus::Trusted),
            "allowed": matches!(p.status, ProviderStatus::Trusted | ProviderStatus::Allowed),
            "denied": matches!(p.status, ProviderStatus::Denied),
            "observation": matches!(p.status, ProviderStatus::Observation),
            "known": true,
        }),
        None => json!({
            "id": null,
            "url": request.provider_url,
            "trusted": false,
            "allowed": false,
            "denied": false,
            "observation": false,
            "known": false,
        }),
    };

    let (recipient_addr, _recipient_chain): (Option<String>, &str) = match &request.destination {
        Destination::X402Endpoint {
            expected_recipient, ..
        } => (expected_recipient.clone(), request.chain.as_str()),
        Destination::Eoa { address } => (Some(address.clone()), request.chain.as_str()),
        Destination::SmartAccount { address } => (Some(address.clone()), request.chain.as_str()),
        Destination::Erc20Transfer { recipient, .. } => {
            (Some(recipient.clone()), request.chain.as_str())
        }
    };

    let recipient = match recipient_addr.as_deref() {
        Some(addr) => match policy
            .recipients
            .iter()
            .find(|r| r.address.eq_ignore_ascii_case(addr) && r.chain == request.chain)
        {
            Some(r) => json!({
                "address": r.address,
                "chain": r.chain,
                "allowed": matches!(r.status, RecipientStatus::Allowed),
                "denied": matches!(r.status, RecipientStatus::Denied),
                "known": true,
            }),
            None => json!({
                "address": addr,
                "chain": request.chain,
                "allowed": false,
                "denied": false,
                "known": false,
            }),
        },
        None => json!({
            "address": null,
            "chain": request.chain,
            "allowed": false,
            "denied": false,
            "known": false,
        }),
    };

    let agent = match policy
        .agents
        .iter()
        .find(|a| a.agent_id == request.agent_id)
    {
        Some(a) => json!({
            "agent_id": a.agent_id,
            "active": matches!(a.status, crate::model::AgentStatus::Active),
            "paused": matches!(a.status, crate::model::AgentStatus::Paused),
            "revoked": matches!(a.status, crate::model::AgentStatus::Revoked),
            "policy_role": a.policy_role.clone().unwrap_or_default(),
            "known": true,
        }),
        None => json!({
            "agent_id": request.agent_id,
            "active": false,
            "paused": false,
            "revoked": false,
            "policy_role": "",
            "known": false,
        }),
    };

    let intent: serde_json::Value = serde_json::to_value(request.intent).unwrap_or(json!(null));
    let payment_protocol: serde_json::Value =
        serde_json::to_value(request.payment_protocol).unwrap_or(json!(null));
    let risk_class: serde_json::Value =
        serde_json::to_value(request.risk_class).unwrap_or(json!(null));

    let paused_agents: Vec<serde_json::Value> = policy
        .emergency
        .paused_agents
        .iter()
        .cloned()
        .map(serde_json::Value::String)
        .collect();

    json!({
        "input": {
            "agent_id": request.agent_id,
            "task_id": request.task_id,
            "intent": intent,
            "payment_protocol": payment_protocol,
            "risk_class": risk_class,
            "amount_usd": amount_usd,
            "token": request.token,
            "chain": request.chain,
            "provider_url": request.provider_url,
            "agent": agent,
            "provider": provider,
            "recipient": recipient,
            "emergency": {
                "freeze_all": policy.emergency.freeze_all,
                "paused_agents": paused_agents,
            }
        }
    })
}

fn same_origin(a: &str, b: &str) -> bool {
    let normalize = |u: &str| u.trim_end_matches('/').to_string();
    let a = normalize(a);
    let b = normalize(b);
    if a == b {
        return true;
    }
    // Match by host: if `b` starts with `a/`, a is the origin.
    if b.starts_with(&format!("{a}/")) || a.starts_with(&format!("{b}/")) {
        return true;
    }
    false
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

    fn aprp(path: &str) -> PaymentRequest {
        let raw = std::fs::read_to_string(path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn golden_request_is_allowed() {
        let p = policy();
        let req = aprp(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../test-corpus/aprp/golden_001_minimal.json"
        ));
        let outcome = decide(&p, &req).unwrap();
        assert_eq!(outcome.decision, Decision::Allow);
        assert_eq!(
            outcome.matched_rule_id.as_deref(),
            Some("allow-small-x402-api-call")
        );
        assert!(outcome.deny_code.is_none());
    }

    #[test]
    fn prompt_injection_is_denied() {
        let p = policy();
        let req = aprp(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../test-corpus/aprp/deny_prompt_injection_request.json"
        ));
        let outcome = decide(&p, &req).unwrap();
        assert_eq!(outcome.decision, Decision::Deny);
        let code = outcome.deny_code.as_deref().unwrap();
        // Either of these is acceptable per `demo-agents/research-agent/README.md`.
        assert!(
            code == "policy.deny_unknown_provider"
                || code == "policy.deny_recipient_not_allowlisted",
            "got deny_code={code}"
        );
    }

    fn golden_aprp() -> PaymentRequest {
        aprp(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../test-corpus/aprp/golden_001_minimal.json"
        ))
    }

    #[test]
    fn unknown_agent_is_denied_before_rule_evaluation() {
        let p = policy();
        let mut req = golden_aprp();
        req.agent_id = "unknown-attacker".into();
        let outcome = decide(&p, &req).unwrap();
        assert_eq!(outcome.decision, Decision::Deny);
        assert_eq!(outcome.deny_code.as_deref(), Some("auth.agent_not_found"));
        assert!(outcome.matched_rule_id.is_none(), "no rule must have fired");
    }

    #[test]
    fn revoked_agent_status_is_denied() {
        let mut p = policy();
        p.agents[0].status = AgentStatus::Revoked;
        let outcome = decide(&p, &golden_aprp()).unwrap();
        assert_eq!(outcome.decision, Decision::Deny);
        assert_eq!(outcome.deny_code.as_deref(), Some("auth.agent_revoked"));
    }

    #[test]
    fn paused_agent_status_is_denied() {
        let mut p = policy();
        p.agents[0].status = AgentStatus::Paused;
        let outcome = decide(&p, &golden_aprp()).unwrap();
        assert_eq!(outcome.decision, Decision::Deny);
        assert_eq!(outcome.deny_code.as_deref(), Some("emergency.agent_paused"));
    }

    #[test]
    fn agent_in_emergency_paused_list_is_denied() {
        let mut p = policy();
        p.emergency.paused_agents.push("research-agent-01".into());
        let outcome = decide(&p, &golden_aprp()).unwrap();
        assert_eq!(outcome.decision, Decision::Deny);
        assert_eq!(outcome.deny_code.as_deref(), Some("emergency.agent_paused"));
    }
}
