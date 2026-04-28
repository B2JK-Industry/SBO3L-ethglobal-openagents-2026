//! Local dev Ed25519 signer + signer-backend trait.
//!
//! Production deployments use a TEE/HSM-backed signer (see
//! `docs/spec/17_interface_contracts.md` §1). This module hosts:
//!
//! - `DevSigner`: the developer-mode Ed25519 signer used by demos and CI.
//! - `SignerBackend`: the trait that decouples Mandate's signing code paths
//!   (`UnsignedReceipt::sign`, `SignedAuditEvent::sign`, `DecisionPayload::sign`)
//!   from any specific backend. `DevSigner` and the production-shaped
//!   `MockKmsSigner` (in `crate::mock_kms`) both implement it. **Neither is a
//!   real KMS or HSM.**

use ed25519_dalek::{Signature, Signer as _, SigningKey, Verifier as _, VerifyingKey};
use rand::rngs::OsRng;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("signature is malformed")]
    BadSignature,
    #[error("public key is malformed")]
    BadPublicKey,
    #[error("hex decoding: {0}")]
    Hex(#[from] hex::FromHexError),
    #[error("signature does not verify")]
    Invalid,
}

#[derive(Debug, Clone)]
pub struct DevSigner {
    pub key_id: String,
    signing_key: SigningKey,
}

impl DevSigner {
    pub fn generate(key_id: impl Into<String>) -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        Self {
            key_id: key_id.into(),
            signing_key,
        }
    }

    /// Deterministic signer from a 32-byte seed. Useful for reproducible demo fixtures.
    pub fn from_seed(key_id: impl Into<String>, seed: [u8; 32]) -> Self {
        Self {
            key_id: key_id.into(),
            signing_key: SigningKey::from_bytes(&seed),
        }
    }

    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    pub fn verifying_key_hex(&self) -> String {
        hex::encode(self.verifying_key().to_bytes())
    }

    pub fn sign(&self, message: &[u8]) -> Signature {
        self.signing_key.sign(message)
    }

    pub fn sign_hex(&self, message: &[u8]) -> String {
        hex::encode(self.sign(message).to_bytes())
    }
}

/// Backend abstraction for the three Mandate signing surfaces (policy
/// receipt, audit event, decision token). Implementors must:
///
/// - report a stable `current_key_id()` that uniquely identifies the
///   key version that produced a signature — verifiers carry this back
///   in the `EmbeddedSignature.key_id` field;
/// - produce hex-encoded Ed25519 signatures over the supplied bytes;
/// - report the verifying key (hex) that corresponds to the
///   *current* signing key, so a verifier can reconstruct trust without
///   reaching into backend internals.
///
/// A backend that supports rotation (e.g. `MockKmsSigner`) MAY also
/// expose historical keys via its own concrete API; the trait itself
/// is intentionally minimal so callers don't depend on lifecycle
/// details that don't apply to every backend.
///
/// **Truthfulness:** implementing this trait does NOT make a backend
/// production-grade. `DevSigner` and `MockKmsSigner` are both local,
/// non-HSM, non-KMS Ed25519 signers; production would replace them
/// with a TEE/HSM-backed implementation behind the same trait.
pub trait SignerBackend {
    fn current_key_id(&self) -> &str;
    fn sign_hex(&self, message: &[u8]) -> String;
    fn current_public_hex(&self) -> String;
}

impl SignerBackend for DevSigner {
    fn current_key_id(&self) -> &str {
        &self.key_id
    }
    fn sign_hex(&self, message: &[u8]) -> String {
        // Inherent method; same body. Keeping the inherent method for
        // direct callers that don't go through the trait.
        DevSigner::sign_hex(self, message)
    }
    fn current_public_hex(&self) -> String {
        self.verifying_key_hex()
    }
}

pub fn verify_hex(
    verifying_key_hex: &str,
    message: &[u8],
    signature_hex: &str,
) -> Result<(), VerifyError> {
    let pk_bytes = hex::decode(verifying_key_hex)?;
    let pk_arr: [u8; 32] = pk_bytes.try_into().map_err(|_| VerifyError::BadPublicKey)?;
    let vk = VerifyingKey::from_bytes(&pk_arr).map_err(|_| VerifyError::BadPublicKey)?;

    let sig_bytes = hex::decode(signature_hex)?;
    let sig_arr: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|_| VerifyError::BadSignature)?;
    let sig = Signature::from_bytes(&sig_arr);

    vk.verify(message, &sig).map_err(|_| VerifyError::Invalid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_verify_round_trip() {
        let s = DevSigner::generate("test-key");
        let msg = b"hello mandate";
        let sig_hex = s.sign_hex(msg);
        verify_hex(&s.verifying_key_hex(), msg, &sig_hex).unwrap();
    }

    #[test]
    fn tampered_message_fails_verification() {
        let s = DevSigner::generate("test-key");
        let sig_hex = s.sign_hex(b"hello");
        let res = verify_hex(&s.verifying_key_hex(), b"goodbye", &sig_hex);
        assert!(matches!(res, Err(VerifyError::Invalid)));
    }

    #[test]
    fn deterministic_seed_yields_stable_pubkey() {
        let seed = [42u8; 32];
        let a = DevSigner::from_seed("k", seed);
        let b = DevSigner::from_seed("k", seed);
        assert_eq!(a.verifying_key_hex(), b.verifying_key_hex());
    }
}
