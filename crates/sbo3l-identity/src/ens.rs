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
    #[error("audit_root on ENS does not match active root ({ens} vs {active})")]
    AuditRootMismatch { ens: String, active: String },
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
    /// URL to the published proof capsule for this agent. Renamed
    /// from `receipt_schema` to match the actually-deployed
    /// `sbo3lagent.eth` text records on Ethereum mainnet (the
    /// previous name pointed at a static JSON Schema; the new name
    /// points at a specific deployed proof instance, which is the
    /// useful thing for an agent registry).
    #[serde(rename = "sbo3l:proof_uri")]
    pub proof_uri: String,
}

/// Strip an optional `0x` / `0X` prefix. Hex records on ENS
/// idiomatically carry the prefix; SBO3L's internal hash format
/// does not. Comparisons accept either form.
fn strip_0x(s: &str) -> &str {
    s.strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s)
}

impl EnsRecords {
    /// Compare ENS-published policy hash against SBO3L's active hash.
    /// Either side may carry a `0x` prefix; comparison is
    /// prefix-tolerant and case-insensitive (hex `a` == `A`).
    pub fn verify_policy_hash(&self, active: &str) -> Result<(), ResolveError> {
        if !strip_0x(&self.policy_hash).eq_ignore_ascii_case(strip_0x(active)) {
            return Err(ResolveError::PolicyHashMismatch {
                ens: self.policy_hash.clone(),
                active: active.to_string(),
            });
        }
        Ok(())
    }

    /// Compare ENS-published audit root against SBO3L's active root.
    /// Same prefix/case tolerance as [`Self::verify_policy_hash`].
    pub fn verify_audit_root(&self, active: &str) -> Result<(), ResolveError> {
        if !strip_0x(&self.audit_root).eq_ignore_ascii_case(strip_0x(active)) {
            return Err(ResolveError::AuditRootMismatch {
                ens: self.audit_root.clone(),
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
                "sbo3l:proof_uri": "https://b2jk-industry.github.io/SBO3L-ethglobal-openagents-2026/capsule.json"
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

    /// Hex records on ENS idiomatically carry a `0x` prefix; SBO3L's
    /// internal hashes do not. Both `verify_policy_hash` and
    /// `verify_audit_root` accept either form on either side, plus
    /// case-insensitive hex.
    #[test]
    fn verify_methods_normalize_0x_prefix_and_case() {
        let r = OfflineEnsResolver::from_json(fixture()).unwrap();
        let mut rec = r.resolve("research-agent.team.eth").unwrap();
        let bare_policy = rec.policy_hash.clone();
        let prefixed_policy = format!("0x{}", bare_policy);
        let upper_policy = bare_policy.to_uppercase();

        // ens=bare, active=0x-prefixed → match
        assert!(rec.verify_policy_hash(&prefixed_policy).is_ok());
        // ens=0x-prefixed, active=bare → match
        rec.policy_hash = prefixed_policy.clone();
        assert!(rec.verify_policy_hash(&bare_policy).is_ok());
        // case-insensitive
        assert!(rec.verify_policy_hash(&upper_policy).is_ok());
        // mismatch still fails (with `0x` on the wrong value)
        let err = rec.verify_policy_hash("0xdeadbeef").unwrap_err();
        assert!(matches!(err, ResolveError::PolicyHashMismatch { .. }));

        // Same checks for audit_root, including the
        // sbo3lagent.eth-shaped value `0x000…000` (66 chars).
        let bare_root = rec.audit_root.clone();
        let prefixed_root = format!("0x{}", bare_root);
        rec.audit_root = prefixed_root.clone();
        assert!(rec.verify_audit_root(&bare_root).is_ok());
        assert!(rec.verify_audit_root(&prefixed_root).is_ok());
        let err = rec.verify_audit_root("ffff").unwrap_err();
        assert!(matches!(err, ResolveError::AuditRootMismatch { .. }));
    }
}
