//! Verifiable audit export bundle (v1).
//!
//! A self-contained, machine-readable proof that a Mandate decision happened
//! and is internally consistent. The bundle pulls together everything a
//! third party (or a future you) needs to verify that:
//!
//! - the policy receipt was signed by the recorded receipt-signer key,
//! - the audit event referenced by the receipt was signed by the recorded
//!   audit-signer key,
//! - the audit event sits in a hash-chained log whose `prev_event_hash`
//!   linkage and per-event `event_hash` reproduce from the canonical event
//!   bytes,
//! - and the receipt's `audit_event_id` actually maps to the supplied
//!   audit event.
//!
//! The bundle is *not* an oracle of legitimacy — it does not say "Mandate
//! issued this receipt"; only the public keys you decide to trust can do
//! that. The bundle says "given that you trust these two public keys,
//! every signature, hash and link in this proof is valid".
//!
//! Tagline: **Mandate does not just decide. It leaves behind verifiable proof.**

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::audit::{verify_chain, ChainError, SignedAuditEvent};
use crate::receipt::{Decision, PolicyReceipt};
use crate::signer::VerifyError;

/// Top-level bundle envelope. See module docs.
///
/// `audit_chain_segment` MUST start at the genesis event (seq=1) and run
/// in seq order through the event referenced by `receipt.audit_event_id`.
/// A future revision can carry a Merkle proof instead of the full segment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditBundle {
    pub bundle_type: BundleType,
    pub version: u32,
    pub exported_at: DateTime<Utc>,
    pub receipt: PolicyReceipt,
    pub audit_event: SignedAuditEvent,
    pub audit_chain_segment: Vec<SignedAuditEvent>,
    pub verification_keys: VerificationKeys,
    pub summary: BundleSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BundleType {
    #[serde(rename = "mandate.audit_bundle.v1")]
    AuditBundleV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationKeys {
    pub receipt_signer_pubkey_hex: String,
    pub audit_signer_pubkey_hex: String,
}

/// Pre-extracted convenience fields. Always derived from the other bundle
/// fields; `verify` re-derives and asserts equality, so a tampered summary
/// cannot lie about the receipt or chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleSummary {
    pub decision: Decision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deny_code: Option<String>,
    pub request_hash: String,
    pub policy_hash: String,
    pub audit_event_id: String,
    pub audit_event_hash: String,
    pub audit_chain_root: String,
    pub audit_chain_latest: String,
}

/// Result of a successful verification. Mirrors the bundle summary plus a
/// few derived counters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifySummary {
    pub receipt_signature_ok: bool,
    pub audit_event_signature_ok: bool,
    pub audit_chain_ok: bool,
    pub receipt_audit_link_ok: bool,
    pub decision: Decision,
    pub deny_code: Option<String>,
    pub request_hash: String,
    pub policy_hash: String,
    pub audit_event_id: String,
    pub audit_event_hash: String,
    pub audit_chain_length: usize,
}

#[derive(Debug, Error)]
pub enum BundleError {
    #[error("bundle is missing a receipt's audit_event_id from the chain segment")]
    AuditEventNotInChain,
    #[error("receipt.audit_event_id does not match audit_event.event.id")]
    ReceiptAuditMismatch,
    #[error("audit_event hash in chain does not match standalone audit_event")]
    AuditEventHashMismatch,
    #[error("summary field '{0}' does not match the bundle body")]
    SummaryMismatch(&'static str),
    #[error("receipt signature does not verify under verification_keys.receipt_signer_pubkey_hex")]
    ReceiptSignatureInvalid,
    #[error(
        "audit_event signature does not verify under verification_keys.audit_signer_pubkey_hex"
    )]
    AuditEventSignatureInvalid,
    #[error("audit chain invalid: {0}")]
    Chain(#[from] ChainError),
    #[error("signer error: {0}")]
    Signer(#[from] VerifyError),
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
}

/// Build a bundle from already-signed pieces. Both signer public keys must
/// be supplied — the bundle records who you intend the verifier to trust,
/// not who actually signed (signatures themselves prove that).
///
/// `audit_chain_segment` must include the receipt's audit event and every
/// preceding event back to seq=1, in seq order.
pub fn build(
    receipt: PolicyReceipt,
    audit_chain_segment: Vec<SignedAuditEvent>,
    receipt_signer_pubkey_hex: String,
    audit_signer_pubkey_hex: String,
    exported_at: DateTime<Utc>,
) -> Result<AuditBundle, BundleError> {
    let audit_event = audit_chain_segment
        .iter()
        .find(|e| e.event.id == receipt.audit_event_id)
        .cloned()
        .ok_or(BundleError::AuditEventNotInChain)?;
    let chain_root = audit_chain_segment
        .first()
        .map(|e| e.event_hash.clone())
        .ok_or(BundleError::AuditEventNotInChain)?;
    let chain_latest = audit_chain_segment
        .last()
        .map(|e| e.event_hash.clone())
        .ok_or(BundleError::AuditEventNotInChain)?;
    let summary = BundleSummary {
        decision: receipt.decision.clone(),
        deny_code: receipt.deny_code.clone(),
        request_hash: receipt.request_hash.clone(),
        policy_hash: receipt.policy_hash.clone(),
        audit_event_id: audit_event.event.id.clone(),
        audit_event_hash: audit_event.event_hash.clone(),
        audit_chain_root: chain_root,
        audit_chain_latest: chain_latest,
    };
    Ok(AuditBundle {
        bundle_type: BundleType::AuditBundleV1,
        version: 1,
        exported_at,
        receipt,
        audit_event,
        audit_chain_segment,
        verification_keys: VerificationKeys {
            receipt_signer_pubkey_hex,
            audit_signer_pubkey_hex,
        },
        summary,
    })
}

/// Verify every claim the bundle makes. Returns a populated `VerifySummary`
/// on success or the first invariant violation as an error. We deliberately
/// fail fast — partial-success reporting would let a tampered bundle pick
/// which checks the verifier sees pass.
///
/// The summary block carried inside the bundle is re-derived and compared
/// against the body, so a tampered summary cannot misrepresent the receipt
/// or chain. (The acceptance test for this is
/// `verify_fails_when_summary_lies_about_decision`.)
pub fn verify(bundle: &AuditBundle) -> Result<VerifySummary, BundleError> {
    // 1. Receipt signature — covers request_hash, policy_hash, decision,
    //    deny_code, audit_event_id, etc. via canonical-body signing.
    bundle
        .receipt
        .verify(&bundle.verification_keys.receipt_signer_pubkey_hex)
        .map_err(|_| BundleError::ReceiptSignatureInvalid)?;

    // 2. Standalone audit_event signature.
    bundle
        .audit_event
        .verify_signature(&bundle.verification_keys.audit_signer_pubkey_hex)
        .map_err(|_| BundleError::AuditEventSignatureInvalid)?;

    // 3. Chain integrity — recomputes every event_hash, walks prev_event_hash,
    //    re-verifies every signature with the same audit signer key.
    verify_chain(
        &bundle.audit_chain_segment,
        true,
        Some(&bundle.verification_keys.audit_signer_pubkey_hex),
    )?;

    // 4. The receipt must point at an event that actually exists in the
    //    chain segment, and the standalone audit_event must match the chain
    //    member with the same id (id, hash, signature, prev pointer all the
    //    same — equality on the SignedAuditEvent struct).
    if bundle.receipt.audit_event_id != bundle.audit_event.event.id {
        return Err(BundleError::ReceiptAuditMismatch);
    }
    let chain_member = bundle
        .audit_chain_segment
        .iter()
        .find(|e| e.event.id == bundle.audit_event.event.id)
        .ok_or(BundleError::AuditEventNotInChain)?;
    if chain_member != &bundle.audit_event {
        // Same id but different signed contents — the standalone event
        // disagrees with the chain member.
        return Err(BundleError::AuditEventHashMismatch);
    }

    // 5. Summary block must agree with the body. Cheap protection against
    //    a tampered summary that contradicts what the (signed) receipt says.
    let s = &bundle.summary;
    if s.decision != bundle.receipt.decision {
        return Err(BundleError::SummaryMismatch("decision"));
    }
    if s.deny_code != bundle.receipt.deny_code {
        return Err(BundleError::SummaryMismatch("deny_code"));
    }
    if s.request_hash != bundle.receipt.request_hash {
        return Err(BundleError::SummaryMismatch("request_hash"));
    }
    if s.policy_hash != bundle.receipt.policy_hash {
        return Err(BundleError::SummaryMismatch("policy_hash"));
    }
    if s.audit_event_id != bundle.audit_event.event.id {
        return Err(BundleError::SummaryMismatch("audit_event_id"));
    }
    if s.audit_event_hash != bundle.audit_event.event_hash {
        return Err(BundleError::SummaryMismatch("audit_event_hash"));
    }
    let expected_root = &bundle
        .audit_chain_segment
        .first()
        .ok_or(BundleError::AuditEventNotInChain)?
        .event_hash;
    if &s.audit_chain_root != expected_root {
        return Err(BundleError::SummaryMismatch("audit_chain_root"));
    }
    let expected_latest = &bundle
        .audit_chain_segment
        .last()
        .ok_or(BundleError::AuditEventNotInChain)?
        .event_hash;
    if &s.audit_chain_latest != expected_latest {
        return Err(BundleError::SummaryMismatch("audit_chain_latest"));
    }

    Ok(VerifySummary {
        receipt_signature_ok: true,
        audit_event_signature_ok: true,
        audit_chain_ok: true,
        receipt_audit_link_ok: true,
        decision: bundle.receipt.decision.clone(),
        deny_code: bundle.receipt.deny_code.clone(),
        request_hash: bundle.receipt.request_hash.clone(),
        policy_hash: bundle.receipt.policy_hash.clone(),
        audit_event_id: bundle.audit_event.event.id.clone(),
        audit_event_hash: bundle.audit_event.event_hash.clone(),
        audit_chain_length: bundle.audit_chain_segment.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::{AuditEvent, ZERO_HASH};
    use crate::receipt::UnsignedReceipt;
    use crate::signer::DevSigner;

    /// Build a small but realistic bundle covering the receipt for seq=2
    /// in a 3-event chain. Exposes the two signers so tampering tests can
    /// flip pieces and re-verify.
    fn fixture() -> (AuditBundle, DevSigner, DevSigner) {
        let audit_signer = DevSigner::from_seed("audit-signer-v1", [11u8; 32]);
        let receipt_signer = DevSigner::from_seed("decision-signer-v1", [7u8; 32]);

        let e1_event = AuditEvent {
            version: 1,
            seq: 1,
            id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGQ".to_string(),
            ts: chrono::DateTime::parse_from_rfc3339("2026-04-27T12:00:00Z")
                .unwrap()
                .into(),
            event_type: "runtime_started".to_string(),
            actor: "mandate-server".to_string(),
            subject_id: "runtime".to_string(),
            payload_hash: ZERO_HASH.to_string(),
            metadata: serde_json::Map::new(),
            policy_version: None,
            policy_hash: None,
            attestation_ref: None,
            prev_event_hash: ZERO_HASH.to_string(),
        };
        let e1 = SignedAuditEvent::sign(e1_event, &audit_signer).unwrap();

        let e2_event = AuditEvent {
            version: 1,
            seq: 2,
            id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGR".to_string(),
            ts: chrono::DateTime::parse_from_rfc3339("2026-04-27T12:00:01Z")
                .unwrap()
                .into(),
            event_type: "policy_decided".to_string(),
            actor: "policy_engine".to_string(),
            subject_id: "pr-test-001".to_string(),
            payload_hash: "1111111111111111111111111111111111111111111111111111111111111111"
                .to_string(),
            metadata: serde_json::Map::new(),
            policy_version: Some(1),
            policy_hash: Some(
                "2222222222222222222222222222222222222222222222222222222222222222".to_string(),
            ),
            attestation_ref: None,
            prev_event_hash: e1.event_hash.clone(),
        };
        let e2 = SignedAuditEvent::sign(e2_event, &audit_signer).unwrap();

        let e3_event = AuditEvent {
            version: 1,
            seq: 3,
            id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGS".to_string(),
            ts: chrono::DateTime::parse_from_rfc3339("2026-04-27T12:00:02Z")
                .unwrap()
                .into(),
            event_type: "policy_decided".to_string(),
            actor: "policy_engine".to_string(),
            subject_id: "pr-test-002".to_string(),
            payload_hash: "3333333333333333333333333333333333333333333333333333333333333333"
                .to_string(),
            metadata: serde_json::Map::new(),
            policy_version: Some(1),
            policy_hash: Some(
                "2222222222222222222222222222222222222222222222222222222222222222".to_string(),
            ),
            attestation_ref: None,
            prev_event_hash: e2.event_hash.clone(),
        };
        let e3 = SignedAuditEvent::sign(e3_event, &audit_signer).unwrap();

        let unsigned = UnsignedReceipt {
            agent_id: "research-agent-01".to_string(),
            decision: Decision::Allow,
            deny_code: None,
            request_hash: "1111111111111111111111111111111111111111111111111111111111111111"
                .to_string(),
            policy_hash: "2222222222222222222222222222222222222222222222222222222222222222"
                .to_string(),
            policy_version: Some(1),
            audit_event_id: e2.event.id.clone(),
            execution_ref: None,
            issued_at: chrono::DateTime::parse_from_rfc3339("2026-04-27T12:00:01.500Z")
                .unwrap()
                .into(),
            expires_at: None,
        };
        let receipt = unsigned.sign(&receipt_signer).unwrap();
        let exported_at: DateTime<Utc> =
            chrono::DateTime::parse_from_rfc3339("2026-04-28T08:00:00Z")
                .unwrap()
                .into();
        let bundle = build(
            receipt,
            vec![e1, e2, e3],
            receipt_signer.verifying_key_hex(),
            audit_signer.verifying_key_hex(),
            exported_at,
        )
        .unwrap();
        (bundle, receipt_signer, audit_signer)
    }

    #[test]
    fn happy_path_round_trip_verifies() {
        let (bundle, _, _) = fixture();
        let summary = verify(&bundle).expect("bundle must verify");
        assert!(summary.receipt_signature_ok);
        assert!(summary.audit_event_signature_ok);
        assert!(summary.audit_chain_ok);
        assert!(summary.receipt_audit_link_ok);
        assert_eq!(summary.audit_chain_length, 3);
        assert_eq!(summary.decision, Decision::Allow);
    }

    #[test]
    fn bundle_canonical_export_is_deterministic() {
        // Two identical inputs must produce byte-identical JSON. We use the
        // standard serde_json::to_vec because the bundle's serde derives
        // serialise fields in a fixed declaration order.
        let (bundle, _, _) = fixture();
        let a = serde_json::to_vec(&bundle).unwrap();
        let b = serde_json::to_vec(&bundle).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn verify_fails_when_request_hash_mutated() {
        // Mutating any signature-covered field on the receipt invalidates
        // the receipt signature. This pins the security claim: a tampered
        // request_hash cannot pass verification even if the signature bytes
        // are kept intact.
        let (mut bundle, _, _) = fixture();
        bundle.receipt.request_hash =
            "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef".to_string();
        // Note: we deliberately do NOT touch the summary here; the receipt-
        // signature check fails before the summary mismatch check would run.
        let err = verify(&bundle).expect_err("must reject mutated request_hash");
        assert!(matches!(err, BundleError::ReceiptSignatureInvalid));
    }

    #[test]
    fn verify_fails_when_policy_hash_mutated() {
        let (mut bundle, _, _) = fixture();
        bundle.receipt.policy_hash =
            "cafebabecafebabecafebabecafebabecafebabecafebabecafebabecafebabe".to_string();
        let err = verify(&bundle).expect_err("must reject mutated policy_hash");
        assert!(matches!(err, BundleError::ReceiptSignatureInvalid));
    }

    #[test]
    fn verify_fails_when_receipt_signature_bytes_mutated() {
        let (mut bundle, _, _) = fixture();
        // Flip one nibble in the signature — must invalidate it without
        // changing any field the signature covers.
        let sig = &mut bundle.receipt.signature.signature_hex;
        let last = sig.pop().unwrap();
        sig.push(if last == '0' { '1' } else { '0' });
        let err = verify(&bundle).expect_err("must reject mutated signature");
        assert!(matches!(err, BundleError::ReceiptSignatureInvalid));
    }

    #[test]
    fn verify_fails_when_audit_event_hash_mutated() {
        // The standalone audit_event must match the chain member of the
        // same id. Mutating the standalone event's hash makes the standalone
        // and the chain disagree — caught by the AuditEventHashMismatch
        // check before chain verification would even matter.
        let (mut bundle, _, _) = fixture();
        bundle.audit_event.event_hash =
            "0000000000000000000000000000000000000000000000000000000000000001".to_string();
        let err = verify(&bundle).expect_err("must reject mutated audit_event hash");
        // Standalone audit_event signature is computed over the *event*, not
        // event_hash; flipping event_hash alone passes signature verify but
        // breaks the standalone vs chain equality check.
        assert!(matches!(err, BundleError::AuditEventHashMismatch));
    }

    #[test]
    fn verify_fails_when_audit_chain_linkage_broken() {
        let (mut bundle, _, _) = fixture();
        // Flip prev_event_hash on seq=3 — verify_chain detects PrevHashMismatch.
        bundle.audit_chain_segment[2].event.prev_event_hash =
            "0000000000000000000000000000000000000000000000000000000000000001".to_string();
        let err = verify(&bundle).expect_err("must reject broken chain linkage");
        assert!(matches!(err, BundleError::Chain(_)));
    }

    #[test]
    fn verify_fails_when_audit_event_not_in_chain() {
        let (mut bundle, _, _) = fixture();
        // Drop the receipt's referenced event from the chain segment.
        bundle.audit_chain_segment.retain(|e| e.event.seq != 2);
        // Patch summary's chain endpoints to keep the summary self-consistent
        // so we exercise the audit-link check, not the summary check.
        bundle.summary.audit_chain_root = bundle.audit_chain_segment[0].event_hash.clone();
        bundle.summary.audit_chain_latest = bundle
            .audit_chain_segment
            .last()
            .unwrap()
            .event_hash
            .clone();
        let err = verify(&bundle).expect_err("must reject missing audit_event");
        // The chain segment now skips seq=2, so prev_event_hash on the
        // remaining seq=3 no longer matches its predecessor — chain verify
        // catches that first. (If the receipt's audit_event_id had pointed
        // outside any plausible chain, the AuditEventNotInChain branch
        // would fire instead. This test pins the realistic path.)
        assert!(matches!(err, BundleError::Chain(_)));
    }

    #[test]
    fn verify_fails_when_summary_lies_about_decision() {
        // Independent property: the summary cannot disagree with the body.
        let (mut bundle, _, _) = fixture();
        bundle.summary.decision = Decision::Deny;
        let err = verify(&bundle).expect_err("must reject summary that lies");
        assert!(matches!(err, BundleError::SummaryMismatch("decision")));
    }

    #[test]
    fn verify_fails_when_wrong_pubkey_supplied() {
        // If the caller swaps the verification key (e.g. pretends a
        // different signer issued the receipt), receipt verification fails.
        let (mut bundle, _, _) = fixture();
        let other = DevSigner::from_seed("attacker", [99u8; 32]);
        bundle.verification_keys.receipt_signer_pubkey_hex = other.verifying_key_hex();
        let err = verify(&bundle).expect_err("must reject wrong receipt pubkey");
        assert!(matches!(err, BundleError::ReceiptSignatureInvalid));
    }
}
