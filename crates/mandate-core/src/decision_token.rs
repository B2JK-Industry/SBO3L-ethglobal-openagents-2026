//! Decision token v1.
//!
//! Mirrors `schemas/decision_token_v1.json`. The signature is over the
//! canonical JSON of the inner `payload` object.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::hashing::canonical_json;
use crate::receipt::Decision;
use crate::signer::{verify_hex, DevSigner, VerifyError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TxTemplate {
    pub chain_id: u64,
    pub to: String,
    pub value: String,
    pub data: String,
    pub gas_limit: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_fee_per_gas: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_priority_fee_per_gas: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonce_hint: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionPayload {
    pub version: u32,
    pub request_hash: String,
    pub decision: Decision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deny_code: Option<String>,
    pub policy_version: u32,
    pub policy_hash: String,
    pub tx_template: TxTemplate,
    pub key_id: String,
    pub decision_id: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attestation_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionToken {
    pub payload: DecisionPayload,
    pub signature_hex: String,
    pub signing_pubkey_hex: String,
}

impl DecisionPayload {
    pub fn sign(self, signer: &DevSigner) -> Result<DecisionToken> {
        let value = serde_json::to_value(&self)?;
        let bytes = canonical_json(&value)?;
        let sig_hex = signer.sign_hex(&bytes);
        Ok(DecisionToken {
            payload: self,
            signature_hex: sig_hex,
            signing_pubkey_hex: signer.verifying_key_hex(),
        })
    }
}

impl DecisionToken {
    pub fn verify(&self) -> std::result::Result<(), VerifyError> {
        let value = serde_json::to_value(&self.payload).map_err(|_| VerifyError::Invalid)?;
        let bytes = canonical_json(&value).map_err(|_| VerifyError::Invalid)?;
        verify_hex(&self.signing_pubkey_hex, &bytes, &self.signature_hex)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_payload() -> DecisionPayload {
        DecisionPayload {
            version: 1,
            request_hash: "c0bd2fab4a7d4686d686edcc9c8356315cd66b820a2072493bf758a1eeb500db"
                .to_string(),
            decision: Decision::Allow,
            deny_code: None,
            policy_version: 1,
            policy_hash: "3333333333333333333333333333333333333333333333333333333333333333"
                .to_string(),
            tx_template: TxTemplate {
                chain_id: 8453,
                to: "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".to_string(),
                value: "0".to_string(),
                data: "0xa9059cbb".to_string(),
                gas_limit: 100_000,
                max_fee_per_gas: Some("1000000000".to_string()),
                max_priority_fee_per_gas: Some("100000000".to_string()),
                nonce_hint: None,
            },
            key_id: "agent-research-01-key".to_string(),
            decision_id: "dec-01HTAWX5K3R8YV9NQB7C6P2DGS".to_string(),
            issued_at: chrono::DateTime::parse_from_rfc3339("2026-04-27T12:00:00Z")
                .unwrap()
                .into(),
            expires_at: chrono::DateTime::parse_from_rfc3339("2026-04-27T12:05:00Z")
                .unwrap()
                .into(),
            attestation_ref: None,
        }
    }

    #[test]
    fn sign_verify_round_trip() {
        let signer = DevSigner::from_seed("decision-signer-v1", [9u8; 32]);
        let token = sample_payload().sign(&signer).unwrap();
        token.verify().unwrap();
    }

    #[test]
    fn signed_token_validates_against_schema() {
        let signer = DevSigner::from_seed("decision-signer-v1", [9u8; 32]);
        let token = sample_payload().sign(&signer).unwrap();
        let value = serde_json::to_value(&token).unwrap();
        crate::schema::validate_decision_token(&value).unwrap();
    }

    #[test]
    fn tampered_payload_fails_verification() {
        let signer = DevSigner::from_seed("decision-signer-v1", [9u8; 32]);
        let mut token = sample_payload().sign(&signer).unwrap();
        token.payload.decision = Decision::Deny;
        let res = token.verify();
        assert!(matches!(res, Err(VerifyError::Invalid)));
    }
}
