//! `gcp_kms` backend (feature `gcp_kms`).
//!
//! Wraps an Ed25519 key managed in Google Cloud KMS. Same shape as the
//! AWS backend — KMS holds the private key, SBO3L calls
//! `AsymmetricSign` via the Cloud KMS SDK and receives the signature.
//!
//! # Status — F-5 hackathon scope
//!
//! Compile-only stub in the F-5 PR. `google-cloud-kms` is not pulled
//! in yet; the live integration lands in a nightly task once Daniel
//! provisions a GCP KMS test key.
//!
//! # Implementation notes for the follow-up wiring
//!
//! - GCP KMS supports Ed25519 via the
//!   `CryptoKeyVersionAlgorithm::Ed25519` enum.
//! - The resource path is
//!   `projects/{p}/locations/{l}/keyRings/{r}/cryptoKeys/{k}/cryptoKeyVersions/{v}`;
//!   the daemon takes it whole from `SBO3L_GCP_KMS_KEY_NAME`.
//! - `AsymmetricSign` accepts `digest` for sign-after-hash flows; SBO3L
//!   wants raw-message signing so the request uses the `data` field
//!   instead. Verify the SDK exposes that variant for Ed25519 keys.
//! - The public key is fetched via `GetPublicKey`; cache per key_id at
//!   construction time, same as the AWS backend.

use super::{Signer, SignerError};

pub struct GcpKmsSigner {
    key_id: String,
}

impl GcpKmsSigner {
    /// Construct from environment. Required env:
    /// `SBO3L_GCP_KMS_KEY_NAME` (full resource name).
    pub fn from_env(_role: &str) -> Result<Self, SignerError> {
        let key_id = std::env::var("SBO3L_GCP_KMS_KEY_NAME")
            .map_err(|_| SignerError::MissingEnv("SBO3L_GCP_KMS_KEY_NAME"))?;
        if key_id.is_empty() {
            return Err(SignerError::MissingEnv("SBO3L_GCP_KMS_KEY_NAME"));
        }
        Ok(Self { key_id })
    }
}

impl Signer for GcpKmsSigner {
    fn sign_hex(&self, _message: &[u8]) -> Result<String, SignerError> {
        Err(SignerError::Kms(format!(
            "gcp_kms backend ({}) not yet implemented; live integration is a nightly task",
            self.key_id
        )))
    }

    fn verifying_key_hex(&self) -> Result<String, SignerError> {
        Err(SignerError::Kms(format!(
            "gcp_kms backend ({}) not yet implemented; live integration is a nightly task",
            self.key_id
        )))
    }

    fn key_id(&self) -> &str {
        &self.key_id
    }
}
