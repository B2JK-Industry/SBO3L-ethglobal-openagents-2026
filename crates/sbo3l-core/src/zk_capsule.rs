//! ZK-redacted capsule verifier scaffold (R13 P4).
//!
//! Trait + types only. Real Groth16 backend (arkworks /
//! bellman) lands once the circom circuit is authored — see
//! `docs/design/zk-capsule-privacy.md` for the full plan and the
//! rationale for shipping scaffold-only here.
//!
//! This module exists so the rest of the codebase can plumb the
//! verification surface today; the real verifier slots in behind
//! the trait without breaking call sites.
//!
//! ## Feature gate
//!
//! Compiled only with `--features zk_capsule_verifier`. Without
//! the feature, the trait + types still exist (so `Box<dyn ZkCapsuleVerifier>`
//! can flow through configs without breaking the type system); the
//! mock implementation does too. The "real" verifier lives behind
//! the feature so the heavy crypto deps (arkworks ~5MB) only land
//! when explicitly opted in.
//!
//! ## What's NOT in this scaffold
//!
//! - The circom circuit (`docs/design/zk-capsule-privacy.md`
//!   §"Architecture sketch" describes what we'd write).
//! - The trusted-setup ceremony output (verifying key).
//! - Browser integration (snarkjs worker — Dev 3's marketing app).
//! - Real Groth16 verification (the [`MockZkCapsuleVerifier`]
//!   below always returns `Ok(true)` — it's a placeholder for
//!   tests to compile against, NOT a verification claim).
//!
//! ## What IS in this scaffold
//!
//! - [`ZkCapsulePublicInputs`] type — the agreed shape of public
//!   inputs the consumer supplies.
//! - [`ZkCapsuleProof`] type — opaque bytes wrapper for the
//!   Groth16 proof.
//! - [`ZkCapsuleVerifier`] trait — the verification surface
//!   consumers wire against.
//! - [`MockZkCapsuleVerifier`] — test impl, always passes; gated
//!   under `#[cfg(test)]` so a production build can't accidentally
//!   wire it as the real verifier.
//! - 4 unit tests covering the type round-trip + the trait shape.

use serde::{Deserialize, Serialize};

/// Public inputs the consumer supplies. Same shape across the
/// hackathon scaffold and the real verifier — the real verifier
/// constrains these against the proof; the scaffold ignores them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZkCapsulePublicInputs {
    /// 32-byte hex of the SBO3L team signing key the proof verifies
    /// the capsule signature against. Stable per release.
    pub sbo3l_pubkey_hex: String,
    /// 32-byte hex challenge nonce — request-bound so proofs can't
    /// replay.
    pub challenge_hex: String,
    /// Low-bit fingerprint of the capsule type. `0x01` for
    /// swap-style capsules, `0x02` for KH-style, etc. The verifier
    /// uses this to pick the right circuit variant; the agent's
    /// real capsule must match this class.
    pub request_class: u8,
}

/// Opaque proof bytes. Real Groth16 proofs are ~256 bytes; we wrap
/// in a `Vec<u8>` so the trait surface is stable across backends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZkCapsuleProof {
    /// Hex-encoded proof bytes. The encoding shape is
    /// backend-specific (Groth16 in our case, but the trait
    /// abstracts).
    pub proof_hex: String,
    /// Identifier for the verifying key that should validate this
    /// proof. Stable per circuit revision; included so a future
    /// verifier rotation doesn't silently accept proofs from the
    /// wrong circuit.
    pub vk_id: String,
}

/// Verifier-side error surface. Stable across backends.
#[derive(Debug, thiserror::Error)]
pub enum ZkCapsuleVerifyError {
    #[error("verifying key id `{0}` is not registered with this verifier")]
    UnknownVerifyingKey(String),
    #[error("proof is malformed: {0}")]
    MalformedProof(String),
    #[error("public inputs are malformed: {0}")]
    MalformedPublicInputs(String),
    #[error("proof verification failed (proof + inputs do not satisfy circuit)")]
    Invalid,
    #[error("backend not implemented (scaffold)")]
    BackendUnavailable,
}

/// Verification surface. Real Groth16 backend implementations live
/// behind the `zk_capsule_verifier` feature; this scaffold lets
/// callers wire `Box<dyn ZkCapsuleVerifier>` today without crypto
/// deps.
pub trait ZkCapsuleVerifier: Send + Sync {
    /// Verify a proof against the public inputs. Returns `Ok(true)`
    /// if the proof is valid, `Ok(false)` if the circuit is unsatisfied,
    /// or an `Err` for plumbing failures.
    fn verify(
        &self,
        public: &ZkCapsulePublicInputs,
        proof: &ZkCapsuleProof,
    ) -> Result<bool, ZkCapsuleVerifyError>;

    /// Stable identifier for this verifier's verifying key.
    fn vk_id(&self) -> &str;
}

/// Mock verifier for unit tests. **Always returns `Ok(true)`** —
/// it's NOT a cryptographic claim. Gated `#[cfg(test)]` so a
/// production build can't accidentally wire it.
#[cfg(test)]
pub struct MockZkCapsuleVerifier {
    pub vk_id: String,
}

#[cfg(test)]
impl MockZkCapsuleVerifier {
    pub fn new(vk_id: impl Into<String>) -> Self {
        Self {
            vk_id: vk_id.into(),
        }
    }
}

#[cfg(test)]
impl ZkCapsuleVerifier for MockZkCapsuleVerifier {
    fn verify(
        &self,
        _public: &ZkCapsulePublicInputs,
        proof: &ZkCapsuleProof,
    ) -> Result<bool, ZkCapsuleVerifyError> {
        // Even the mock checks vk_id — proofs from the wrong vk_id
        // are rejected. Catches "wrong verifier wired" bugs in
        // calling code.
        if proof.vk_id != self.vk_id {
            return Err(ZkCapsuleVerifyError::UnknownVerifyingKey(
                proof.vk_id.clone(),
            ));
        }
        Ok(true)
    }

    fn vk_id(&self) -> &str {
        &self.vk_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_public() -> ZkCapsulePublicInputs {
        ZkCapsulePublicInputs {
            sbo3l_pubkey_hex: "0".repeat(64),
            challenge_hex: "1".repeat(64),
            request_class: 0x01,
        }
    }

    fn fixture_proof(vk_id: &str) -> ZkCapsuleProof {
        ZkCapsuleProof {
            proof_hex: "deadbeef".repeat(64), // 256 bytes hex
            vk_id: vk_id.to_string(),
        }
    }

    #[test]
    fn public_inputs_round_trip_via_json() {
        let p = fixture_public();
        let s = serde_json::to_string(&p).unwrap();
        let back: ZkCapsulePublicInputs = serde_json::from_str(&s).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn proof_round_trip_via_json() {
        let p = fixture_proof("vk-v1");
        let s = serde_json::to_string(&p).unwrap();
        let back: ZkCapsuleProof = serde_json::from_str(&s).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn mock_verifier_accepts_matching_vk() {
        let v = MockZkCapsuleVerifier::new("vk-v1");
        let r = v
            .verify(&fixture_public(), &fixture_proof("vk-v1"))
            .unwrap();
        assert!(r);
    }

    #[test]
    fn mock_verifier_rejects_mismatched_vk() {
        let v = MockZkCapsuleVerifier::new("vk-v1");
        let err = v
            .verify(&fixture_public(), &fixture_proof("vk-v2"))
            .unwrap_err();
        assert!(matches!(err, ZkCapsuleVerifyError::UnknownVerifyingKey(_)));
    }

    #[test]
    fn deny_unknown_fields_in_public_inputs() {
        let bad = r#"{
            "sbo3l_pubkey_hex": "00",
            "challenge_hex": "11",
            "request_class": 1,
            "extra": "rejected"
        }"#;
        let res: Result<ZkCapsulePublicInputs, _> = serde_json::from_str(bad);
        assert!(res.is_err());
    }
}
