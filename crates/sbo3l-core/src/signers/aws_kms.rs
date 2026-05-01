//! `aws_kms` backend (feature `aws_kms`).
//!
//! Wraps an Ed25519 key managed in AWS KMS. The KMS holds the private
//! key in its HSM; SBO3L sends signature requests via the AWS SDK and
//! receives the signature bytes back. Public key material is fetched
//! once at construction and cached.
//!
//! # Status — F-5 hackathon scope
//!
//! This module ships as a **compile-only stub** in the F-5 PR. The
//! `aws-sdk-kms` crate dependency is intentionally NOT pulled in yet:
//! the daemon's startup path can route to this backend (and surface the
//! "not yet implemented" error), but `cargo build --features aws_kms`
//! does not pull a multi-MB SDK tree until the implementation actually
//! uses it. The integration test against a real AWS KMS test key lands
//! in a follow-up nightly task once Daniel provisions the key — see
//! `docs/win-backlog/05-phase-1.md` F-5 review checklist.
//!
//! # Implementation notes for the follow-up wiring
//!
//! - AWS KMS supports Ed25519 via `KeySpec::Ed25519` (verify the
//!   region availability before provisioning).
//! - The signing API is `Sign` with `SigningAlgorithmSpec::EddsaEd25519`;
//!   `MessageType::Raw` matches SBO3L's existing `sign(message: &[u8])`
//!   surface (we hash JCS-canonical bytes ourselves upstream).
//! - The public key fetch is `GetPublicKey`; cache per key_id at
//!   construction time so per-request latency is one round-trip.
//! - The SDK is async — the synchronous [`Signer::sign_hex`] impl will
//!   need a `tokio::runtime::Handle::block_on` shim or a small
//!   blocking thread-pool wrapper. The existing daemon already runs a
//!   tokio runtime so a `Handle::current().block_on(...)` from
//!   `tokio::task::block_in_place` is the cleanest path.

use super::{Signer, SignerError};

/// Production-shaped AWS KMS Ed25519 signer.
///
/// In the F-5 PR this is a stub — the constructor reads
/// `SBO3L_AWS_KMS_KEY_ID` for the key alias / ARN but does not yet open
/// an SDK client. Calls to [`Signer::sign_hex`] return a
/// `SignerError::Kms("aws_kms backend not yet implemented; nightly
/// task")` until the SDK wiring lands in a follow-up PR.
pub struct AwsKmsSigner {
    key_id: String,
}

impl AwsKmsSigner {
    /// Construct from environment. Required env: `SBO3L_AWS_KMS_KEY_ID`
    /// (key alias such as `alias/sbo3l-test` or full ARN). The
    /// `_role` parameter mirrors the dev backend's signature so the
    /// factory call is identical regardless of backend; production
    /// deployments can encode the role in the alias name.
    pub fn from_env(_role: &str) -> Result<Self, SignerError> {
        let key_id = std::env::var("SBO3L_AWS_KMS_KEY_ID")
            .map_err(|_| SignerError::MissingEnv("SBO3L_AWS_KMS_KEY_ID"))?;
        if key_id.is_empty() {
            return Err(SignerError::MissingEnv("SBO3L_AWS_KMS_KEY_ID"));
        }
        Ok(Self { key_id })
    }
}

impl Signer for AwsKmsSigner {
    fn sign_hex(&self, _message: &[u8]) -> Result<String, SignerError> {
        Err(SignerError::Kms(format!(
            "aws_kms backend ({}) not yet implemented; live integration is a nightly task",
            self.key_id
        )))
    }

    fn verifying_key_hex(&self) -> Result<String, SignerError> {
        Err(SignerError::Kms(format!(
            "aws_kms backend ({}) not yet implemented; live integration is a nightly task",
            self.key_id
        )))
    }

    fn key_id(&self) -> &str {
        &self.key_id
    }
}
