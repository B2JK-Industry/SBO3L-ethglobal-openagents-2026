//! Audit event v1 — protocol types and chain helpers.
//!
//! Mirrors `schemas/audit_event_v1.json`. The signature is over the canonical
//! JSON of the inner `event` object.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::hashing::{canonical_json, sha256_hex};
use crate::receipt::EmbeddedSignature as Signature;
use crate::receipt::SignatureAlgorithm;
use crate::signer::{verify_hex, DevSigner, VerifyError};

pub const ZERO_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditEvent {
    pub version: u32,
    pub seq: u64,
    pub id: String,
    pub ts: DateTime<Utc>,
    #[serde(rename = "type")]
    pub event_type: String,
    pub actor: String,
    pub subject_id: String,
    pub payload_hash: String,
    pub metadata: serde_json::Map<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_version: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attestation_ref: Option<String>,
    pub prev_event_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedAuditEvent {
    pub event: AuditEvent,
    pub event_hash: String,
    pub signature: Signature,
}

#[derive(Debug, thiserror::Error)]
pub enum ChainError {
    #[error("seq out of order at index {index}: expected {expected}, got {got}")]
    SeqOutOfOrder {
        index: usize,
        expected: u64,
        got: u64,
    },
    #[error("prev_event_hash mismatch at seq {seq}")]
    PrevHashMismatch { seq: u64 },
    #[error("event_hash mismatch at seq {seq}")]
    EventHashMismatch { seq: u64 },
    #[error("signature verification failed at seq {seq}")]
    SignatureFailed { seq: u64 },
    #[error(transparent)]
    Core(#[from] crate::error::CoreError),
}

impl AuditEvent {
    pub fn canonical_hash(&self) -> Result<String> {
        let v = serde_json::to_value(self)?;
        let bytes = canonical_json(&v)?;
        Ok(sha256_hex(&bytes))
    }
}

impl SignedAuditEvent {
    /// Sign an `AuditEvent` and return the signed envelope. The signature is
    /// computed over the canonical JSON of the inner event; the same bytes are
    /// used to derive `event_hash`.
    pub fn sign(event: AuditEvent, signer: &DevSigner) -> Result<Self> {
        let v = serde_json::to_value(&event)?;
        let bytes = canonical_json(&v)?;
        let event_hash = sha256_hex(&bytes);
        let sig_hex = signer.sign_hex(&bytes);
        Ok(Self {
            event,
            event_hash,
            signature: Signature {
                algorithm: SignatureAlgorithm::Ed25519,
                key_id: signer.key_id.clone(),
                signature_hex: sig_hex,
            },
        })
    }

    pub fn verify_signature(
        &self,
        verifying_key_hex: &str,
    ) -> std::result::Result<(), VerifyError> {
        let v = serde_json::to_value(&self.event).map_err(|_| VerifyError::Invalid)?;
        let bytes = canonical_json(&v).map_err(|_| VerifyError::Invalid)?;
        verify_hex(verifying_key_hex, &bytes, &self.signature.signature_hex)
    }
}

/// Verify the integrity of a chain of signed audit events.
///
/// Checks performed:
///   * `seq` starts at 1 and is monotonic.
///   * `prev_event_hash` of each event matches the prior event's `event_hash`
///     (or `ZERO_HASH` for the genesis event).
///   * if `verify_hashes` is `true`, recomputes `event_hash` from canonical
///     event bytes and compares.
///   * if `verifying_key_hex` is provided, verifies each event's signature.
pub fn verify_chain(
    events: &[SignedAuditEvent],
    verify_hashes: bool,
    verifying_key_hex: Option<&str>,
) -> std::result::Result<(), ChainError> {
    let mut prev_hash = ZERO_HASH.to_string();
    for (i, signed) in events.iter().enumerate() {
        let expected_seq = (i as u64) + 1;
        if signed.event.seq != expected_seq {
            return Err(ChainError::SeqOutOfOrder {
                index: i,
                expected: expected_seq,
                got: signed.event.seq,
            });
        }
        if signed.event.prev_event_hash != prev_hash {
            return Err(ChainError::PrevHashMismatch {
                seq: signed.event.seq,
            });
        }
        if verify_hashes {
            let computed = signed.event.canonical_hash()?;
            if computed != signed.event_hash {
                return Err(ChainError::EventHashMismatch {
                    seq: signed.event.seq,
                });
            }
        }
        if let Some(pk) = verifying_key_hex {
            if signed.verify_signature(pk).is_err() {
                return Err(ChainError::SignatureFailed {
                    seq: signed.event.seq,
                });
            }
        }
        prev_hash = signed.event_hash.clone();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ev(seq: u64, prev: &str, ts: &str) -> AuditEvent {
        // ULIDs are exactly 26 Crockford base32 chars after "evt-".
        let suffix = match seq {
            1 => "01HTAWX5K3R8YV9NQB7C6P2DGQ",
            2 => "01HTAWX5K3R8YV9NQB7C6P2DGR",
            3 => "01HTAWX5K3R8YV9NQB7C6P2DGS",
            _ => "01HTAWX5K3R8YV9NQB7C6P2DGZ",
        };
        AuditEvent {
            version: 1,
            seq,
            id: format!("evt-{suffix}"),
            ts: chrono::DateTime::parse_from_rfc3339(ts).unwrap().into(),
            event_type: "runtime_started".to_string(),
            actor: "mandate-server".to_string(),
            subject_id: "runtime".to_string(),
            payload_hash: ZERO_HASH.to_string(),
            metadata: json!({"mode":"dev"}).as_object().unwrap().clone(),
            policy_version: None,
            policy_hash: None,
            attestation_ref: None,
            prev_event_hash: prev.to_string(),
        }
    }

    #[test]
    fn sign_and_verify_envelope() {
        let signer = DevSigner::from_seed("audit-signer-v1", [13u8; 32]);
        let signed = SignedAuditEvent::sign(ev(1, ZERO_HASH, "2026-04-27T12:00:00Z"), &signer)
            .expect("sign");
        signed
            .verify_signature(&signer.verifying_key_hex())
            .unwrap();
    }

    #[test]
    fn signed_event_validates_against_schema() {
        let signer = DevSigner::from_seed("audit-signer-v1", [13u8; 32]);
        let signed = SignedAuditEvent::sign(ev(1, ZERO_HASH, "2026-04-27T12:00:00Z"), &signer)
            .expect("sign");
        let v = serde_json::to_value(&signed).unwrap();
        crate::schema::validate_audit_event(&v).unwrap();
    }

    #[test]
    fn chain_verify_round_trip() {
        let signer = DevSigner::from_seed("audit-signer-v1", [13u8; 32]);
        let e1 = SignedAuditEvent::sign(ev(1, ZERO_HASH, "2026-04-27T12:00:00Z"), &signer).unwrap();
        let e2 =
            SignedAuditEvent::sign(ev(2, &e1.event_hash, "2026-04-27T12:00:01Z"), &signer).unwrap();
        let e3 =
            SignedAuditEvent::sign(ev(3, &e2.event_hash, "2026-04-27T12:00:02Z"), &signer).unwrap();
        verify_chain(&[e1, e2, e3], true, Some(&signer.verifying_key_hex())).unwrap();
    }

    #[test]
    fn chain_verify_detects_tamper_in_middle() {
        let signer = DevSigner::from_seed("audit-signer-v1", [13u8; 32]);
        let e1 = SignedAuditEvent::sign(ev(1, ZERO_HASH, "2026-04-27T12:00:00Z"), &signer).unwrap();
        let mut e2 =
            SignedAuditEvent::sign(ev(2, &e1.event_hash, "2026-04-27T12:00:01Z"), &signer).unwrap();
        // Mutate event without re-signing.
        e2.event.actor = "attacker".to_string();
        let err = verify_chain(&[e1.clone(), e2.clone()], true, None).unwrap_err();
        assert!(matches!(err, ChainError::EventHashMismatch { .. }));
    }
}
