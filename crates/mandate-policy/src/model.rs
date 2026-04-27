//! Policy YAML/JSON model. Mirrors `schemas/policy_v1.json`.

use serde::{Deserialize, Serialize};

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

impl Policy {
    pub fn parse_yaml(s: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(s)
    }

    pub fn parse_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
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
}
