//! Policy receipt v1.
//!
//! Mirrors `schemas/policy_receipt_v1.json`. The receipt is signed over the
//! canonical JSON of the receipt with the `signature` field omitted.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::hashing::canonical_json;
use crate::signer::{verify_hex, SignerBackend, VerifyError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Allow,
    Deny,
    RequiresHuman,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbeddedSignature {
    pub algorithm: SignatureAlgorithm,
    pub key_id: String,
    pub signature_hex: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SignatureAlgorithm {
    Ed25519,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyReceipt {
    pub receipt_type: ReceiptType,
    pub version: u32,
    pub agent_id: String,
    pub decision: Decision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deny_code: Option<String>,
    pub request_hash: String,
    pub policy_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_version: Option<u32>,
    pub audit_event_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_ref: Option<String>,
    pub issued_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    pub signature: EmbeddedSignature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReceiptType {
    #[serde(rename = "sbo3l.policy_receipt.v1")]
    PolicyReceiptV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsignedReceipt {
    pub agent_id: String,
    pub decision: Decision,
    pub deny_code: Option<String>,
    pub request_hash: String,
    pub policy_hash: String,
    pub policy_version: Option<u32>,
    pub audit_event_id: String,
    pub execution_ref: Option<String>,
    pub issued_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl UnsignedReceipt {
    /// Sign the receipt with any [`SignerBackend`] — `DevSigner` for
    /// the existing demo path, `MockKmsSigner` for the production-shaped
    /// path. The receipt's `signature.key_id` is taken from the
    /// backend's `current_key_id()` so verifiers can route the lookup.
    pub fn sign<S: SignerBackend + ?Sized>(self, signer: &S) -> Result<PolicyReceipt> {
        // Build the receipt with a placeholder signature so we can compute the
        // canonical body bytes by stripping the `signature` key from the JSON.
        let placeholder = PolicyReceipt {
            receipt_type: ReceiptType::PolicyReceiptV1,
            version: 1,
            agent_id: self.agent_id,
            decision: self.decision,
            deny_code: self.deny_code,
            request_hash: self.request_hash,
            policy_hash: self.policy_hash,
            policy_version: self.policy_version,
            audit_event_id: self.audit_event_id,
            execution_ref: self.execution_ref,
            issued_at: self.issued_at,
            expires_at: self.expires_at,
            signature: EmbeddedSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                key_id: signer.current_key_id().to_string(),
                signature_hex: "placeholder".to_string(),
            },
        };
        let bytes = canonicalize_body(&placeholder)?;
        let sig_hex = signer.sign_hex(&bytes);
        Ok(PolicyReceipt {
            signature: EmbeddedSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                key_id: signer.current_key_id().to_string(),
                signature_hex: sig_hex,
            },
            ..placeholder
        })
    }
}

impl PolicyReceipt {
    pub fn verify(&self, verifying_key_hex: &str) -> std::result::Result<(), VerifyError> {
        let bytes = canonicalize_body(self).map_err(|_| VerifyError::Invalid)?;
        verify_hex(verifying_key_hex, &bytes, &self.signature.signature_hex)
    }
}

fn canonicalize_body(receipt: &PolicyReceipt) -> Result<Vec<u8>> {
    let mut value = serde_json::to_value(receipt)?;
    if let Some(obj) = value.as_object_mut() {
        obj.remove("signature");
    }
    canonical_json(&value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signer::DevSigner;

    fn unsigned() -> UnsignedReceipt {
        UnsignedReceipt {
            agent_id: "research-agent-01".to_string(),
            decision: Decision::Allow,
            deny_code: None,
            request_hash: "1111111111111111111111111111111111111111111111111111111111111111"
                .to_string(),
            policy_hash: "2222222222222222222222222222222222222222222222222222222222222222"
                .to_string(),
            policy_version: Some(1),
            audit_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGS".to_string(),
            execution_ref: None,
            issued_at: chrono::DateTime::parse_from_rfc3339("2026-04-27T12:00:00Z")
                .unwrap()
                .into(),
            expires_at: None,
        }
    }

    #[test]
    fn sign_and_verify_round_trip() {
        let signer = DevSigner::from_seed("decision-signer-v1", [7u8; 32]);
        let receipt = unsigned().sign(&signer).unwrap();
        receipt.verify(&signer.verifying_key_hex()).unwrap();
    }

    #[test]
    fn tampered_receipt_fails_verification() {
        let signer = DevSigner::from_seed("decision-signer-v1", [7u8; 32]);
        let mut receipt = unsigned().sign(&signer).unwrap();
        receipt.agent_id = "attacker-agent".to_string();
        let result = receipt.verify(&signer.verifying_key_hex());
        assert!(matches!(result, Err(VerifyError::Invalid)));
    }

    #[test]
    fn signed_receipt_validates_against_schema() {
        let signer = DevSigner::from_seed("decision-signer-v1", [7u8; 32]);
        let receipt = unsigned().sign(&signer).unwrap();
        let value = serde_json::to_value(&receipt).unwrap();
        crate::schema::validate_policy_receipt(&value).unwrap();
    }
}
