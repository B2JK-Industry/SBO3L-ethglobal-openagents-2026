//! Local dev Ed25519 signer.
//!
//! Production deployments use a TEE/HSM-backed signer (see
//! `docs/spec/17_interface_contracts.md` §1). This module is the developer-mode
//! backend used by demos and CI.

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
