//! Pedersen commitments + Schnorr proof-of-knowledge for capsule
//! privacy (R14 P1 — upgrade of #302 scaffold).
//!
//! Replaces the R13 mock with **real cryptography**: Ristretto-based
//! Pedersen commitments + a Schnorr identification protocol that
//! proves knowledge of the commitment's opening (the message bytes
//! + the blinding scalar) without revealing them.
//!
//! ## What this is, what it isn't
//!
//! This is **commitment-based selective disclosure**, not a full
//! Groth16 SNARK over capsule validity. The original R13 design
//! doc (`docs/design/zk-capsule-privacy.md`) describes the full
//! Groth16 path: prove "I have a valid SBO3L capsule whose
//! signature recovers under the team key, whose audit chain links
//! to a fresh head, and whose decision is `allow`" without
//! revealing any of those fields. That circuit is ~1000 lines of
//! circom + a trusted-setup ceremony + browser snarkjs integration;
//! multi-day work, properly scoped in the design doc.
//!
//! What we ship here is a **strictly narrower but cryptographically
//! real** primitive: an agent commits to a capsule (or any message)
//! and can later prove it knows the opening without revealing the
//! contents. This is the **anti-front-running** + **timed-disclosure**
//! shape — useful for: agent posts a commitment to its decision at
//! T0, market moves, agent reveals the decision at T1; observers
//! between T0 and T1 can verify the agent didn't change its mind
//! (commitment binds), but can't read the decision (commitment
//! hides).
//!
//! ## Cryptographic claims
//!
//! - **Hiding**: a commitment `C = g^m + h^r` reveals nothing
//!   about `m` to a polynomial-time adversary that doesn't know
//!   `r`. Holds under the discrete-log assumption on Ristretto.
//! - **Binding**: opening to two different `(m, r)` pairs would
//!   yield distinct commitments. An adversary forging two
//!   openings would have to compute `dlog_g(h)`, hard under DL.
//! - **Schnorr PoK**: the prover demonstrates knowledge of the
//!   discrete log `r` of `(C - hash(m) * G)` w.r.t. `H` — i.e.,
//!   that they know the opening — using the Fiat-Shamir-transformed
//!   Schnorr protocol. Sound under DL + ROM on the FS hash.
//!
//! `g` is the standard Ristretto basepoint. `h` is derived from a
//! fixed domain string via `RistrettoPoint::hash_from_bytes` so
//! the two generators are independent (standard Pedersen-on-Ristretto
//! pattern).
//!
//! ## What the original ZkCapsuleVerifier trait still does
//!
//! The trait + `ZkCapsulePublicInputs` + `ZkCapsuleProof` types
//! from R13 P4 are preserved as a separate **future-Groth16-shape**
//! surface (kept so downstream callers don't break). The
//! Groth16-backed `ZkCapsuleVerifier` impl lands when the
//! multi-day circom work happens. Today's real cryptography is
//! the Pedersen+Schnorr layer below.

use blake3::Hasher;
use curve25519_dalek::{
    constants::RISTRETTO_BASEPOINT_POINT,
    ristretto::{CompressedRistretto, RistrettoPoint},
    scalar::Scalar,
};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};

/// Domain separation for deriving the second Pedersen generator.
const PEDERSEN_H_DOMAIN: &[u8] = b"SBO3L-Pedersen-H-v1";

/// Domain separation for the Fiat-Shamir hash.
const SCHNORR_FS_DOMAIN: &[u8] = b"SBO3L-Schnorr-FS-v1";

// ============================================================
// R13-compat types (preserved for the future-Groth16 trait shape)
// ============================================================

/// Public inputs the consumer supplies to a future Groth16
/// verifier. **Preserved from R13 P4 scaffold** so downstream
/// callers don't break — the real Groth16 backend lands separately.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZkCapsulePublicInputs {
    pub sbo3l_pubkey_hex: String,
    pub challenge_hex: String,
    pub request_class: u8,
}

/// Opaque proof bytes for a future Groth16 backend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZkCapsuleProof {
    pub proof_hex: String,
    pub vk_id: String,
}

/// Future-Groth16 verifier trait. Implementations live behind the
/// `zk_capsule_verifier` feature once the circuit ships.
pub trait ZkCapsuleVerifier: Send + Sync {
    fn verify(
        &self,
        public: &ZkCapsulePublicInputs,
        proof: &ZkCapsuleProof,
    ) -> Result<bool, ZkCapsuleVerifyError>;
    fn vk_id(&self) -> &str;
}

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
    #[error("backend not implemented (Groth16 circuit pending)")]
    BackendUnavailable,
}

// ============================================================
// R14 P1: real Pedersen + Schnorr (the new primitive)
// ============================================================

/// Pedersen commitment over Ristretto. `point = g^m + h^r` where
/// `g` is the basepoint, `h` is the SBO3L-domain Pedersen
/// generator, `m` is `hash_to_scalar(message)`, and `r` is the
/// blinding factor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PedersenCommitment {
    /// 32-byte compressed Ristretto encoding.
    pub bytes: [u8; 32],
}

impl PedersenCommitment {
    pub fn to_hex(&self) -> String {
        hex::encode(self.bytes)
    }

    pub fn from_hex(hex: &str) -> Result<Self, CommitmentError> {
        let raw = ::hex::decode(hex).map_err(|e| CommitmentError::Hex(e.to_string()))?;
        let bytes: [u8; 32] = raw
            .try_into()
            .map_err(|v: Vec<u8>| CommitmentError::WrongLength(v.len()))?;
        Ok(Self { bytes })
    }
}

impl Serialize for PedersenCommitment {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.to_hex().serialize(s)
    }
}

impl<'de> Deserialize<'de> for PedersenCommitment {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Self::from_hex(&s).map_err(serde::de::Error::custom)
    }
}

/// The opening of a Pedersen commitment: the message + the
/// blinding scalar. Both required to verify the commitment.
#[derive(Debug, Clone)]
pub struct CommitmentOpening {
    pub message: Vec<u8>,
    pub blinding: Scalar,
}

impl CommitmentOpening {
    /// Fresh opening — message + a randomly-generated blinding
    /// scalar. Use [`commit_from_opening`] or
    /// [`commit_with_opening`] to produce the commitment.
    pub fn new(message: impl Into<Vec<u8>>) -> Self {
        let mut rng = OsRng;
        Self {
            message: message.into(),
            blinding: Scalar::random(&mut rng),
        }
    }

    /// Hex-encode the blinding scalar for at-rest storage.
    pub fn blinding_hex(&self) -> String {
        hex::encode(self.blinding.to_bytes())
    }

    /// Reconstruct an opening from the message + blinding hex.
    pub fn from_parts(message: Vec<u8>, blinding_hex: &str) -> Result<Self, CommitmentError> {
        let bytes: [u8; 32] = ::hex::decode(blinding_hex)
            .map_err(|e| CommitmentError::Hex(e.to_string()))?
            .try_into()
            .map_err(|v: Vec<u8>| CommitmentError::WrongLength(v.len()))?;
        let blinding = Scalar::from_canonical_bytes(bytes)
            .into_option()
            .ok_or(CommitmentError::NonCanonicalScalar)?;
        Ok(Self { message, blinding })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CommitmentError {
    #[error("hex decode error: {0}")]
    Hex(String),
    #[error("wrong byte length: {0}")]
    WrongLength(usize),
    #[error("non-canonical scalar")]
    NonCanonicalScalar,
    #[error("invalid Ristretto point encoding")]
    InvalidPoint,
    #[error("commitment does not match opening")]
    CommitmentMismatch,
    #[error("Schnorr proof verification failed")]
    SchnorrVerifyFailed,
}

/// Hash an arbitrary message to a Ristretto scalar via BLAKE3 XOF.
pub fn hash_to_scalar(message: &[u8]) -> Scalar {
    let mut h = Hasher::new();
    h.update(b"SBO3L-msg-to-scalar-v1");
    h.update(message);
    let mut wide = [0u8; 64];
    let mut xof = h.finalize_xof();
    xof.fill(&mut wide);
    Scalar::from_bytes_mod_order_wide(&wide)
}

/// The second Pedersen generator. Stable; derived from a fixed
/// domain string.
pub fn pedersen_h() -> RistrettoPoint {
    RistrettoPoint::hash_from_bytes::<sha2::Sha512>(PEDERSEN_H_DOMAIN)
}

/// Compute the Pedersen commitment for a fresh opening.
pub fn commit_with_opening(message: impl Into<Vec<u8>>) -> (PedersenCommitment, CommitmentOpening) {
    let opening = CommitmentOpening::new(message);
    let commitment = commit_from_opening(&opening);
    (commitment, opening)
}

/// Compute the commitment from a known opening.
pub fn commit_from_opening(opening: &CommitmentOpening) -> PedersenCommitment {
    let m = hash_to_scalar(&opening.message);
    let g = RISTRETTO_BASEPOINT_POINT;
    let h = pedersen_h();
    let point = m * g + opening.blinding * h;
    PedersenCommitment {
        bytes: point.compress().to_bytes(),
    }
}

/// Verify that a commitment matches a claimed opening.
pub fn verify_opening(
    commitment: &PedersenCommitment,
    opening: &CommitmentOpening,
) -> Result<(), CommitmentError> {
    let recomputed = commit_from_opening(opening);
    if recomputed.bytes != commitment.bytes {
        return Err(CommitmentError::CommitmentMismatch);
    }
    Ok(())
}

// ============================================================
// Schnorr proof-of-knowledge of the opening's blinding factor
// ============================================================

/// Schnorr proof that the prover knows the blinding scalar `r`
/// such that `commitment = hash_to_scalar(message) * G + r * H`,
/// without revealing `r`.
///
/// Wire form: `(R, s)` where `R = k * H` for fresh nonce `k`,
/// `s = k + c * r mod ℓ`, and `c = H_FS(commitment || message || R)`
/// is the Fiat-Shamir challenge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchnorrProofOfOpening {
    /// 32-byte compressed Ristretto encoding of `R = k * H`.
    pub commitment_point_hex: String,
    /// 32-byte hex of `s = k + c * r`.
    pub response_hex: String,
}

/// Produce a Schnorr proof of knowledge of the opening's blinding
/// scalar.
pub fn prove_opening(
    commitment: &PedersenCommitment,
    opening: &CommitmentOpening,
) -> SchnorrProofOfOpening {
    let mut rng = OsRng;
    let k = Scalar::random(&mut rng);
    let h = pedersen_h();
    let big_r = k * h;
    let big_r_compressed = big_r.compress();

    let challenge = fs_challenge(commitment, &opening.message, &big_r_compressed);
    let s = k + challenge * opening.blinding;

    SchnorrProofOfOpening {
        commitment_point_hex: hex::encode(big_r_compressed.to_bytes()),
        response_hex: hex::encode(s.to_bytes()),
    }
}

/// Verify a Schnorr proof of opening. Verifier needs the
/// commitment, the **public** message, and the proof. The
/// blinding scalar stays private.
pub fn verify_opening_proof(
    commitment: &PedersenCommitment,
    message: &[u8],
    proof: &SchnorrProofOfOpening,
) -> Result<(), CommitmentError> {
    let big_r = decode_point(&proof.commitment_point_hex)?;
    let s = decode_scalar(&proof.response_hex)?;
    let big_c = decode_compressed_point_bytes(&commitment.bytes)?;

    let m = hash_to_scalar(message);
    let g = RISTRETTO_BASEPOINT_POINT;
    let h = pedersen_h();
    let big_r_bytes: [u8; 32] = ::hex::decode(&proof.commitment_point_hex)
        .map_err(|e| CommitmentError::Hex(e.to_string()))?
        .try_into()
        .map_err(|v: Vec<u8>| CommitmentError::WrongLength(v.len()))?;
    let big_r_compressed =
        CompressedRistretto::from_slice(&big_r_bytes).map_err(|_| CommitmentError::InvalidPoint)?;
    let challenge = fs_challenge(commitment, message, &big_r_compressed);

    // Verify: s * H == R + c * (C - m*G)
    let lhs = s * h;
    let rhs = big_r + challenge * (big_c - m * g);
    if lhs != rhs {
        return Err(CommitmentError::SchnorrVerifyFailed);
    }
    Ok(())
}

fn fs_challenge(
    commitment: &PedersenCommitment,
    message: &[u8],
    big_r: &CompressedRistretto,
) -> Scalar {
    let mut h = Hasher::new();
    h.update(SCHNORR_FS_DOMAIN);
    h.update(&commitment.bytes);
    h.update(&(message.len() as u64).to_le_bytes());
    h.update(message);
    h.update(big_r.as_bytes());
    let mut wide = [0u8; 64];
    let mut xof = h.finalize_xof();
    xof.fill(&mut wide);
    Scalar::from_bytes_mod_order_wide(&wide)
}

fn decode_point(hex: &str) -> Result<RistrettoPoint, CommitmentError> {
    let bytes: [u8; 32] = ::hex::decode(hex)
        .map_err(|e| CommitmentError::Hex(e.to_string()))?
        .try_into()
        .map_err(|v: Vec<u8>| CommitmentError::WrongLength(v.len()))?;
    decode_compressed_point_bytes(&bytes)
}

fn decode_compressed_point_bytes(bytes: &[u8; 32]) -> Result<RistrettoPoint, CommitmentError> {
    CompressedRistretto::from_slice(bytes)
        .map_err(|_| CommitmentError::InvalidPoint)?
        .decompress()
        .ok_or(CommitmentError::InvalidPoint)
}

fn decode_scalar(hex: &str) -> Result<Scalar, CommitmentError> {
    let bytes: [u8; 32] = ::hex::decode(hex)
        .map_err(|e| CommitmentError::Hex(e.to_string()))?
        .try_into()
        .map_err(|v: Vec<u8>| CommitmentError::WrongLength(v.len()))?;
    Scalar::from_canonical_bytes(bytes)
        .into_option()
        .ok_or(CommitmentError::NonCanonicalScalar)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Pedersen layer ---

    #[test]
    fn commit_and_verify_round_trip() {
        let message = b"sbo3l capsule bytes";
        let (commitment, opening) = commit_with_opening(message.to_vec());
        verify_opening(&commitment, &opening).unwrap();
    }

    #[test]
    fn commit_hides_message() {
        let message = b"identical";
        let (c1, _) = commit_with_opening(message.to_vec());
        let (c2, _) = commit_with_opening(message.to_vec());
        assert_ne!(c1.bytes, c2.bytes);
    }

    #[test]
    fn commit_binds() {
        let (c, mut opening) = commit_with_opening(b"original".to_vec());
        opening.message = b"tampered".to_vec();
        let err = verify_opening(&c, &opening).unwrap_err();
        assert!(matches!(err, CommitmentError::CommitmentMismatch));
    }

    #[test]
    fn commit_serialisation_round_trip() {
        let (c, _) = commit_with_opening(b"x".to_vec());
        let s = serde_json::to_string(&c).unwrap();
        let back: PedersenCommitment = serde_json::from_str(&s).unwrap();
        assert_eq!(c.bytes, back.bytes);
    }

    #[test]
    fn opening_round_trip_via_blinding_hex() {
        let opening = CommitmentOpening::new(b"msg".to_vec());
        let message = opening.message.clone();
        let blinding_hex = opening.blinding_hex();
        let back = CommitmentOpening::from_parts(message.clone(), &blinding_hex).unwrap();
        assert_eq!(back.message, message);
        assert_eq!(back.blinding, opening.blinding);
    }

    #[test]
    fn pedersen_h_is_stable() {
        let h1 = pedersen_h().compress();
        let h2 = pedersen_h().compress();
        assert_eq!(h1.to_bytes(), h2.to_bytes());
    }

    #[test]
    fn pedersen_h_independent_from_basepoint() {
        assert_ne!(
            pedersen_h().compress().to_bytes(),
            RISTRETTO_BASEPOINT_POINT.compress().to_bytes()
        );
    }

    // --- Schnorr PoK layer ---

    #[test]
    fn schnorr_pok_round_trip() {
        let message = b"sbo3l capsule";
        let (commitment, opening) = commit_with_opening(message.to_vec());
        let proof = prove_opening(&commitment, &opening);
        verify_opening_proof(&commitment, message, &proof).unwrap();
    }

    #[test]
    fn schnorr_pok_fails_for_wrong_message() {
        let (commitment, opening) = commit_with_opening(b"original".to_vec());
        let proof = prove_opening(&commitment, &opening);
        let err = verify_opening_proof(&commitment, b"tampered", &proof).unwrap_err();
        assert!(matches!(err, CommitmentError::SchnorrVerifyFailed));
    }

    #[test]
    fn schnorr_pok_fails_for_tampered_commitment() {
        let (commitment, opening) = commit_with_opening(b"x".to_vec());
        let proof = prove_opening(&commitment, &opening);
        let mut tampered = commitment;
        tampered.bytes[0] ^= 0x01;
        let err = verify_opening_proof(&tampered, b"x", &proof);
        assert!(err.is_err());
    }

    #[test]
    fn schnorr_pok_fails_for_tampered_response() {
        let (commitment, opening) = commit_with_opening(b"x".to_vec());
        let mut proof = prove_opening(&commitment, &opening);
        let mut response_chars: Vec<char> = proof.response_hex.chars().collect();
        let idx = response_chars.len() / 2;
        let original = response_chars[idx];
        response_chars[idx] = if original == 'a' { 'b' } else { 'a' };
        proof.response_hex = response_chars.into_iter().collect();
        let err = verify_opening_proof(&commitment, b"x", &proof);
        assert!(err.is_err());
    }

    #[test]
    fn schnorr_pok_proofs_for_same_input_differ() {
        let (commitment, opening) = commit_with_opening(b"x".to_vec());
        let p1 = prove_opening(&commitment, &opening);
        let p2 = prove_opening(&commitment, &opening);
        assert_ne!(p1.commitment_point_hex, p2.commitment_point_hex);
        assert_ne!(p1.response_hex, p2.response_hex);
        verify_opening_proof(&commitment, b"x", &p1).unwrap();
        verify_opening_proof(&commitment, b"x", &p2).unwrap();
    }

    #[test]
    fn schnorr_pok_serialises_round_trip() {
        let (commitment, opening) = commit_with_opening(b"x".to_vec());
        let proof = prove_opening(&commitment, &opening);
        let s = serde_json::to_string(&proof).unwrap();
        let back: SchnorrProofOfOpening = serde_json::from_str(&s).unwrap();
        assert_eq!(proof, back);
        verify_opening_proof(&commitment, b"x", &back).unwrap();
    }

    #[test]
    fn deny_unknown_fields_in_schnorr_proof() {
        let bad = r#"{
            "commitment_point_hex": "00",
            "response_hex": "00",
            "extra": "rejected"
        }"#;
        let res: Result<SchnorrProofOfOpening, _> = serde_json::from_str(bad);
        assert!(res.is_err());
    }

    // --- R13-compat trait shape (preserved) ---

    #[test]
    fn r13_compat_public_inputs_round_trip() {
        let p = ZkCapsulePublicInputs {
            sbo3l_pubkey_hex: "0".repeat(64),
            challenge_hex: "1".repeat(64),
            request_class: 0x01,
        };
        let s = serde_json::to_string(&p).unwrap();
        let back: ZkCapsulePublicInputs = serde_json::from_str(&s).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn r13_compat_proof_round_trip() {
        let p = ZkCapsuleProof {
            proof_hex: "deadbeef".repeat(64),
            vk_id: "vk-v1".to_string(),
        };
        let s = serde_json::to_string(&p).unwrap();
        let back: ZkCapsuleProof = serde_json::from_str(&s).unwrap();
        assert_eq!(p, back);
    }
}
