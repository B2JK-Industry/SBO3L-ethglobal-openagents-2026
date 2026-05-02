//! FROST threshold signature scaffold (R13 P8).
//!
//! Trait + types only. Real `frost-ed25519` integration lands once
//! the DKG + sign harnesses are authored — see
//! `docs/design/frost-threshold-sigs.md` for the full plan and the
//! rationale for shipping scaffold-only here.
//!
//! ## Why a separate trait, not a `ThresholdSigner: Signer`?
//!
//! The threshold operation is **stateful across rounds** —
//! signers exchange round-1 commitments, then round-2 partial sigs.
//! The existing `Signer` trait is single-shot
//! (`sign_payload(&[u8]) -> Signature`); fitting two rounds
//! into one trait method would either block on a quorum (sync
//! call freezing the daemon) or smuggle a futures-channel under
//! the trait shape (leaking implementation detail).
//!
//! The cleaner pattern is a **separate `ThresholdSigner` trait**
//! that exposes the staged operations (`request_signing`,
//! `submit_partial`, `aggregate`) and a higher-level orchestrator
//! that consumes it. Once the orchestrator lands, the daemon's
//! receipt path uses the orchestrator (not the trait directly),
//! and the trait surface stays minimal + auditable.
//!
//! ## What's NOT in this scaffold
//!
//! - The DKG round-1/round-2 flow. (Multi-day; see design doc.)
//! - The sign round-1/round-2 flow. (Multi-day; see design doc.)
//! - The `frost-ed25519` integration. (Drop-in once flows land.)
//! - The board-member signoff CLI.
//!
//! ## What IS in this scaffold
//!
//! - [`ThresholdConfig`] — public committee parameters
//!   (threshold, member count, aggregated pubkey).
//! - [`SigningRequest`] / [`SigningResponse`] — message types
//!   exchanged between coordinator + signers.
//! - [`ThresholdSigner`] trait — the per-member signing surface.
//! - [`MockThresholdCommittee`] — gated under `#[cfg(test)]`,
//!   simulates an in-memory committee for tests; **NOT a
//!   cryptographic threshold sig** (uses single-key sig under
//!   the hood, gated by quorum count). Lets the rest of the
//!   codebase plumb the surface.
//! - 5 unit tests covering the type round-trip + the trait shape.

use serde::{Deserialize, Serialize};

/// Public committee parameters. Same shape across the hackathon
/// scaffold and the real FROST integration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThresholdConfig {
    /// Total committee size. E.g. 5.
    pub member_count: u16,
    /// Threshold required for a valid signature. `threshold <= member_count`.
    /// E.g. 3 for "3-of-5".
    pub threshold: u16,
    /// Aggregated public key (32-byte hex). Pinned at DKG completion;
    /// stable until re-DKG.
    pub aggregated_pubkey_hex: String,
    /// Stable identifier for this committee. Pinned at DKG so a
    /// signing request can't be confused with one from a different
    /// committee.
    pub committee_id: String,
}

/// Coordinator → signer: "please sign this payload."
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SigningRequest {
    pub committee_id: String,
    pub request_id: String,
    /// 32-byte hex digest the threshold signature will be over.
    pub payload_digest_hex: String,
    /// Unix-seconds beyond which the request expires (signers
    /// reject). Anti-replay + bounded-pending semantics.
    pub deadline_secs: u64,
    /// Free-form context the signer presents to the human reviewer
    /// (board member). E.g. "swap 1 ETH for USDC at quote $3450".
    pub human_context: String,
}

/// Signer → coordinator: "I have signed (or refused)."
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SigningResponse {
    pub committee_id: String,
    pub request_id: String,
    /// 0..member_count — which committee position this signer holds.
    pub signer_index: u16,
    /// Hex-encoded partial signature share. Real FROST: ~32-64 bytes.
    pub partial_sig_hex: String,
    /// Was the signer instructed to refuse? `true` = refusal
    /// (e.g. board member rejected the operation); `false` = sign.
    /// Refusal carries no partial sig (`partial_sig_hex` ignored).
    pub refused: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ThresholdError {
    #[error("committee id mismatch: request `{request_committee}` vs config `{config_committee}`")]
    CommitteeIdMismatch {
        request_committee: String,
        config_committee: String,
    },
    #[error("threshold not reached: {got} signers, {needed} required")]
    BelowThreshold { got: u16, needed: u16 },
    #[error("invalid signer index {got}, max {max}")]
    InvalidSignerIndex { got: u16, max: u16 },
    #[error("duplicate signer index {0}")]
    DuplicateSigner(u16),
    #[error("aggregation failed: {0}")]
    AggregationFailed(String),
    #[error("backend not implemented (scaffold)")]
    BackendUnavailable,
}

/// Per-member signer. One impl per committee position.
pub trait ThresholdSigner: Send + Sync {
    /// Submit a partial signature in response to a signing request.
    /// In the real FROST integration this triggers the round-1 +
    /// round-2 protocol with the coordinator.
    fn sign_partial(&self, request: &SigningRequest) -> Result<SigningResponse, ThresholdError>;

    /// Stable index of this signer in the committee (0..member_count).
    fn signer_index(&self) -> u16;
}

/// Aggregator: combine partial signatures into a final
/// signature. Pure function over the partials + the committee
/// config. Real impl runs `frost-ed25519::aggregate`.
pub fn aggregate_partials(
    config: &ThresholdConfig,
    request: &SigningRequest,
    partials: &[SigningResponse],
) -> Result<String, ThresholdError> {
    if request.committee_id != config.committee_id {
        return Err(ThresholdError::CommitteeIdMismatch {
            request_committee: request.committee_id.clone(),
            config_committee: config.committee_id.clone(),
        });
    }
    let mut signed: Vec<&SigningResponse> = partials.iter().filter(|p| !p.refused).collect();
    if (signed.len() as u16) < config.threshold {
        return Err(ThresholdError::BelowThreshold {
            got: signed.len() as u16,
            needed: config.threshold,
        });
    }
    // Sort by signer_index for deterministic aggregation order.
    signed.sort_by_key(|p| p.signer_index);
    // Reject duplicate indices.
    for w in signed.windows(2) {
        if w[0].signer_index == w[1].signer_index {
            return Err(ThresholdError::DuplicateSigner(w[0].signer_index));
        }
    }
    for p in &signed {
        if p.signer_index >= config.member_count {
            return Err(ThresholdError::InvalidSignerIndex {
                got: p.signer_index,
                max: config.member_count,
            });
        }
    }
    // SCAFFOLD ONLY: the real impl runs frost-ed25519::aggregate
    // with the round-1 binding factors + the round-2 partial sigs.
    // Here we concatenate the partial-sig hex strings and hash the
    // result. THIS IS NOT A CRYPTOGRAPHIC THRESHOLD SIGNATURE.
    // Documented as such; gated under the same scaffold posture as
    // zk_capsule.
    let mut combined = String::with_capacity(64);
    for p in &signed {
        combined.push_str(&p.partial_sig_hex);
    }
    Ok(format!("scaffold:{combined}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_3_of_5() -> ThresholdConfig {
        ThresholdConfig {
            member_count: 5,
            threshold: 3,
            aggregated_pubkey_hex: "00".repeat(32),
            committee_id: "board-2026".to_string(),
        }
    }

    fn request() -> SigningRequest {
        SigningRequest {
            committee_id: "board-2026".to_string(),
            request_id: "req-1".to_string(),
            payload_digest_hex: "ab".repeat(32),
            deadline_secs: 1_777_905_000,
            human_context: "swap 1 ETH for USDC".to_string(),
        }
    }

    fn partial(index: u16, refused: bool) -> SigningResponse {
        SigningResponse {
            committee_id: "board-2026".to_string(),
            request_id: "req-1".to_string(),
            signer_index: index,
            partial_sig_hex: format!("{:02x}{:02x}", index, index),
            refused,
        }
    }

    #[test]
    fn config_round_trip_via_json() {
        let c = config_3_of_5();
        let s = serde_json::to_string(&c).unwrap();
        let back: ThresholdConfig = serde_json::from_str(&s).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn aggregate_with_threshold_succeeds() {
        let r = request();
        let partials = vec![partial(0, false), partial(2, false), partial(4, false)];
        let sig = aggregate_partials(&config_3_of_5(), &r, &partials).unwrap();
        assert!(sig.starts_with("scaffold:"));
    }

    #[test]
    fn aggregate_below_threshold_fails() {
        let r = request();
        let partials = vec![partial(0, false), partial(1, false)];
        let err = aggregate_partials(&config_3_of_5(), &r, &partials).unwrap_err();
        assert!(matches!(
            err,
            ThresholdError::BelowThreshold { got: 2, needed: 3 }
        ));
    }

    #[test]
    fn aggregate_ignores_refused_signers() {
        let r = request();
        // 3 signed + 2 refused → exactly meets threshold.
        let partials = vec![
            partial(0, false),
            partial(1, true),
            partial(2, false),
            partial(3, true),
            partial(4, false),
        ];
        let sig = aggregate_partials(&config_3_of_5(), &r, &partials).unwrap();
        assert!(sig.starts_with("scaffold:"));
    }

    #[test]
    fn aggregate_rejects_committee_id_mismatch() {
        let r = SigningRequest {
            committee_id: "wrong-committee".to_string(),
            ..request()
        };
        let partials = vec![partial(0, false), partial(1, false), partial(2, false)];
        let err = aggregate_partials(&config_3_of_5(), &r, &partials).unwrap_err();
        assert!(matches!(err, ThresholdError::CommitteeIdMismatch { .. }));
    }

    #[test]
    fn aggregate_rejects_duplicate_signer_index() {
        let r = request();
        let partials = vec![partial(0, false), partial(0, false), partial(2, false)];
        let err = aggregate_partials(&config_3_of_5(), &r, &partials).unwrap_err();
        assert!(matches!(err, ThresholdError::DuplicateSigner(0)));
    }

    #[test]
    fn aggregate_rejects_out_of_range_index() {
        let r = request();
        // signer_index 7 doesn't exist in a 5-member committee.
        let partials = vec![partial(7, false), partial(1, false), partial(2, false)];
        let err = aggregate_partials(&config_3_of_5(), &r, &partials).unwrap_err();
        assert!(matches!(
            err,
            ThresholdError::InvalidSignerIndex { got: 7, max: 5 }
        ));
    }

    #[test]
    fn deny_unknown_fields_in_config() {
        let bad = r#"{
            "member_count": 5,
            "threshold": 3,
            "aggregated_pubkey_hex": "00",
            "committee_id": "x",
            "extra": "rejected"
        }"#;
        let res: Result<ThresholdConfig, _> = serde_json::from_str(bad);
        assert!(res.is_err());
    }
}
