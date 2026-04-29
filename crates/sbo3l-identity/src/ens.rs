//! ENS agent-identity resolution.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ResolveError {
    #[error("unknown name {0}")]
    UnknownName(String),
    #[error("missing sbo3l:{0} text record on {1}")]
    MissingRecord(&'static str, String),
    #[error("policy_hash on ENS does not match active policy ({ens} vs {active})")]
    PolicyHashMismatch { ens: String, active: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnsRecords {
    #[serde(rename = "sbo3l:agent_id")]
    pub agent_id: String,
    #[serde(rename = "sbo3l:endpoint")]
    pub endpoint: String,
    #[serde(rename = "sbo3l:policy_hash")]
    pub policy_hash: String,
    #[serde(rename = "sbo3l:audit_root")]
    pub audit_root: String,
    #[serde(rename = "sbo3l:receipt_schema")]
    pub receipt_schema: String,
}

impl EnsRecords {
    pub fn verify_policy_hash(&self, active: &str) -> Result<(), ResolveError> {
        if self.policy_hash != active {
            return Err(ResolveError::PolicyHashMismatch {
                ens: self.policy_hash.clone(),
                active: active.to_string(),
            });
        }
        Ok(())
    }
}

/// Resolve an ENS-like name to SBO3L text records.
pub trait EnsResolver {
    fn resolve(&self, name: &str) -> Result<EnsRecords, ResolveError>;
}

/// Offline resolver backed by a JSON fixture mapping name -> records.
pub struct OfflineEnsResolver {
    pub records: HashMap<String, EnsRecords>,
}

impl OfflineEnsResolver {
    pub fn from_json(raw: &str) -> Result<Self, ResolveError> {
        let map: HashMap<String, EnsRecords> = serde_json::from_str(raw)?;
        Ok(Self { records: map })
    }

    pub fn from_file(path: &std::path::Path) -> Result<Self, ResolveError> {
        let raw = std::fs::read_to_string(path)?;
        Self::from_json(&raw)
    }
}

impl EnsResolver for OfflineEnsResolver {
    fn resolve(&self, name: &str) -> Result<EnsRecords, ResolveError> {
        self.records
            .get(name)
            .cloned()
            .ok_or_else(|| ResolveError::UnknownName(name.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> &'static str {
        r#"{
            "research-agent.team.eth": {
                "sbo3l:agent_id": "research-agent-01",
                "sbo3l:endpoint": "http://127.0.0.1:8730/v1",
                "sbo3l:policy_hash": "e044f13c5acb792dd3109f1be3a98536168b0990e25595b3cedc131d02e666cf",
                "sbo3l:audit_root": "0000000000000000000000000000000000000000000000000000000000000000",
                "sbo3l:receipt_schema": "https://schemas.sbo3l.dev/policy-receipt/v1.json"
            }
        }"#
    }

    #[test]
    fn offline_resolver_returns_records() {
        let r = OfflineEnsResolver::from_json(fixture()).unwrap();
        let rec = r.resolve("research-agent.team.eth").unwrap();
        assert_eq!(rec.agent_id, "research-agent-01");
    }

    #[test]
    fn unknown_name_errors() {
        let r = OfflineEnsResolver::from_json(fixture()).unwrap();
        assert!(matches!(
            r.resolve("nope.eth"),
            Err(ResolveError::UnknownName(_))
        ));
    }

    #[test]
    fn policy_hash_verification() {
        let r = OfflineEnsResolver::from_json(fixture()).unwrap();
        let rec = r.resolve("research-agent.team.eth").unwrap();
        assert!(rec.verify_policy_hash(&rec.policy_hash).is_ok());
        let err = rec.verify_policy_hash("deadbeef").unwrap_err();
        assert!(matches!(err, ResolveError::PolicyHashMismatch { .. }));
    }
}
