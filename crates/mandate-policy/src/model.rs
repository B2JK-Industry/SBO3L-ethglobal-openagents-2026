//! Policy YAML/JSON model. Mirrors `schemas/policy_v1.json`.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub version: u32,
    pub policy_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub default_decision: DefaultDecision,
    pub agents: Vec<AgentSelector>,
    #[serde(default)]
    pub budgets: Vec<Budget>,
    #[serde(default)]
    pub providers: Vec<Provider>,
    #[serde(default)]
    pub recipients: Vec<Recipient>,
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub emergency: Emergency,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefaultDecision {
    Deny,
    RequiresHuman,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSelector {
    pub agent_id: String,
    pub status: AgentStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_role: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Active,
    Paused,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Budget {
    pub agent_id: String,
    pub scope: BudgetScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_key: Option<String>,
    pub cap_usd: String,
    // Reserved: parsed and round-tripped, but not enforced by `BudgetTracker`
    // in this hackathon scope. Production engines (per
    // `docs/spec/17_interface_contracts.md`) emit a soft-cap warning that
    // surfaces in the receipt. Tracked in `SUBMISSION_NOTES.md` → "Known
    // limitations".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub soft_cap_usd: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetScope {
    PerTx,
    Daily,
    Monthly,
    PerProvider,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provider {
    pub id: String,
    pub url: String,
    pub status: ProviderStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cert_pin_sha256: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderStatus {
    Trusted,
    Allowed,
    Denied,
    Observation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipient {
    pub address: String,
    pub chain: String,
    pub status: RecipientStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecipientStatus {
    Allowed,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub id: String,
    pub effect: RuleEffect,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deny_code: Option<String>,
    pub when: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleEffect {
    Allow,
    Deny,
    RequiresHuman,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Emergency {
    #[serde(default)]
    pub freeze_all: bool,
    #[serde(default)]
    pub paused_agents: Vec<String>,
}

/// Errors that can occur when parsing or validating a `Policy`.
///
/// `serde(deny_unknown_fields)` already protects every nested struct against
/// unexpected JSON/YAML keys; the [`PolicyParseError::Validation`] variant
/// covers semantic invariants that serde cannot express, such as uniqueness
/// of `agents[].agent_id`.
#[derive(Debug, Error)]
pub enum PolicyParseError {
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("yaml: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error(transparent)]
    Validation(#[from] PolicyValidationError),
}

/// Semantic validation failures detected after a policy has been deserialised.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PolicyValidationError {
    #[error("agents[].agent_id must be unique; '{0}' appears more than once")]
    DuplicateAgentId(String),
    #[error("rules[].id must be unique; '{0}' appears more than once")]
    DuplicateRuleId(String),
    #[error("providers[].id must be unique; '{0}' appears more than once")]
    DuplicateProviderId(String),
    #[error("recipients[] must be unique by (address, chain); '{address}@{chain}' duplicated")]
    DuplicateRecipient { address: String, chain: String },
}

impl Policy {
    pub fn parse_yaml(s: &str) -> Result<Self, PolicyParseError> {
        let policy: Self = serde_yaml::from_str(s)?;
        policy.validate()?;
        Ok(policy)
    }

    pub fn parse_json(s: &str) -> Result<Self, PolicyParseError> {
        let policy: Self = serde_json::from_str(s)?;
        policy.validate()?;
        Ok(policy)
    }

    /// Run semantic validation that serde cannot express. Called automatically
    /// by [`Policy::parse_json`] and [`Policy::parse_yaml`]; exposed publicly
    /// so callers that build a `Policy` programmatically (tests, fuzzing) can
    /// run the same checks.
    pub fn validate(&self) -> Result<(), PolicyValidationError> {
        let mut agent_ids = HashSet::with_capacity(self.agents.len());
        for a in &self.agents {
            if !agent_ids.insert(a.agent_id.as_str()) {
                return Err(PolicyValidationError::DuplicateAgentId(a.agent_id.clone()));
            }
        }
        let mut rule_ids = HashSet::with_capacity(self.rules.len());
        for r in &self.rules {
            if !rule_ids.insert(r.id.as_str()) {
                return Err(PolicyValidationError::DuplicateRuleId(r.id.clone()));
            }
        }
        let mut provider_ids = HashSet::with_capacity(self.providers.len());
        for p in &self.providers {
            if !provider_ids.insert(p.id.as_str()) {
                return Err(PolicyValidationError::DuplicateProviderId(p.id.clone()));
            }
        }
        let mut recipients = HashSet::with_capacity(self.recipients.len());
        for r in &self.recipients {
            // Addresses are case-insensitive on EVM chains; normalise before
            // comparing so `0xAA…` and `0xaa…` are treated as the same.
            let key = (r.address.to_ascii_lowercase(), r.chain.clone());
            if !recipients.insert(key) {
                return Err(PolicyValidationError::DuplicateRecipient {
                    address: r.address.clone(),
                    chain: r.chain.clone(),
                });
            }
        }
        Ok(())
    }

    /// Canonical SHA-256 hex of the policy. Computed over JCS-canonical JSON.
    pub fn canonical_hash(&self) -> Result<String, serde_json::Error> {
        let v = serde_json::to_value(self)?;
        let bytes = serde_json_canonicalizer::to_string(&v)
            .expect("canonicalisation only fails on invalid floats which Policy never produces")
            .into_bytes();
        use sha2::Digest;
        Ok(hex::encode(sha2::Sha256::digest(&bytes)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_reference_low_risk_fixture() {
        let raw = include_str!("../../../test-corpus/policy/reference_low_risk.json");
        let policy: Policy = Policy::parse_json(raw).unwrap();
        assert_eq!(policy.policy_id, "default-low-risk");
        assert_eq!(policy.default_decision, DefaultDecision::Deny);
        assert_eq!(policy.rules.len(), 4);
        assert_eq!(policy.budgets.len(), 3);
    }

    #[test]
    fn canonical_hash_is_stable() {
        let raw = include_str!("../../../test-corpus/policy/reference_low_risk.json");
        let policy: Policy = Policy::parse_json(raw).unwrap();
        let h1 = policy.canonical_hash().unwrap();
        let h2 = policy.canonical_hash().unwrap();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
    }

    fn base() -> Policy {
        Policy::parse_json(include_str!(
            "../../../test-corpus/policy/reference_low_risk.json"
        ))
        .unwrap()
    }

    #[test]
    fn duplicate_agent_id_is_rejected() {
        let mut p = base();
        let dup = p.agents[0].clone();
        p.agents.push(dup);
        let err = p.validate().expect_err("must reject duplicate agent_id");
        assert!(
            matches!(err, PolicyValidationError::DuplicateAgentId(ref id) if id == "research-agent-01"),
            "got {err:?}"
        );
    }

    #[test]
    fn duplicate_rule_id_is_rejected() {
        let mut p = base();
        let dup = p.rules[0].clone();
        p.rules.push(dup);
        let err = p.validate().expect_err("must reject duplicate rule id");
        assert!(matches!(err, PolicyValidationError::DuplicateRuleId(_)));
    }

    #[test]
    fn duplicate_provider_id_is_rejected() {
        let mut p = base();
        let dup = p.providers[0].clone();
        p.providers.push(dup);
        let err = p.validate().expect_err("must reject duplicate provider id");
        assert!(matches!(err, PolicyValidationError::DuplicateProviderId(_)));
    }

    #[test]
    fn duplicate_recipient_is_rejected_case_insensitively() {
        let mut p = base();
        // Mutate to upper-case to prove the check normalises before comparing.
        let mut dup = p.recipients[0].clone();
        dup.address = dup.address.to_ascii_uppercase();
        p.recipients.push(dup);
        let err = p
            .validate()
            .expect_err("must reject duplicate recipient regardless of address casing");
        assert!(matches!(
            err,
            PolicyValidationError::DuplicateRecipient { .. }
        ));
    }

    #[test]
    fn parse_json_runs_validation() {
        // Drop a manual duplicate into the raw JSON and confirm parse_json
        // surfaces a `Validation` error rather than silently accepting it.
        let raw = include_str!("../../../test-corpus/policy/reference_low_risk.json");
        let mut value: serde_json::Value = serde_json::from_str(raw).unwrap();
        let extra = value["agents"][0].clone();
        value["agents"].as_array_mut().unwrap().push(extra);
        let bad = serde_json::to_string(&value).unwrap();
        let err =
            Policy::parse_json(&bad).expect_err("must reject duplicate agent_id at parse time");
        assert!(
            matches!(
                err,
                PolicyParseError::Validation(PolicyValidationError::DuplicateAgentId(_))
            ),
            "got {err:?}"
        );
    }
}
