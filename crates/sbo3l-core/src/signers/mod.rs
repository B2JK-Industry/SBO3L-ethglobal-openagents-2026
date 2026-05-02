//! F-5: KMS abstraction. The [`Signer`] trait and a runtime factory that
//! selects one of four backends based on the `SBO3L_SIGNER_BACKEND` env
//! var:
//!
//! - `dev` → [`dev::DevSignerLockedDown`] — local Ed25519 with a
//!   production-mode lockout. Refuses to construct unless
//!   `SBO3L_DEV_ONLY_SIGNER=1` is set, and prints a
//!   `⚠ DEV ONLY SIGNER ⚠` stderr banner when it does.
//! - `aws_kms` → [`aws_kms::AwsKmsSigner`] (feature `aws_kms`).
//! - `gcp_kms` → [`gcp_kms::GcpKmsSigner`] (feature `gcp_kms`).
//! - `phala_tee` → [`phala_tee::PhalaTeeSigner`] (feature `phala_tee`,
//!   Phase 3 placeholder; not a real TEE today).
//!
//! All implementations produce **identical Ed25519 wire format** —
//! 64-byte signatures verifiable by [`crate::signer::verify_hex`] against
//! the 32-byte verifying key returned from `verifying_key_hex`. A
//! receipt signed by `AwsKmsSigner` is byte-equivalent on the wire to
//! one signed by `DevSignerLockedDown`; offline verifiers don't need to
//! know which backend produced it.
//!
//! # Cross-team coordination — sibling [`eth::EthSigner`] trait
//!
//! Receipt and audit signing are Ed25519. EVM transaction signing
//! (Dev 4's T-3-1 Durin issuance work) is secp256k1. Different curves,
//! different wire formats; one trait would force callers to disambiguate
//! which key shape they want at every call site. F-5 ships [`Signer`]
//! for Ed25519 and [`eth::EthSigner`] as a sibling for secp256k1, both
//! capable of being backed by the same KMS but with separate key
//! material. Dev 4 wires the actual EVM impl in T-3-1.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SignerError {
    /// `dev` backend was selected without the `SBO3L_DEV_ONLY_SIGNER=1`
    /// production-mode override. The daemon's startup path turns this
    /// into a stderr warning + exit code 2.
    #[error(
        "DEV signer requires SBO3L_DEV_ONLY_SIGNER=1 to be set; \
         this is a production-mode lockout, not a configuration nag"
    )]
    DevOnlyLockout,

    /// `SBO3L_SIGNER_BACKEND` was set to a string the factory doesn't
    /// recognise.
    #[error(
        "unknown SBO3L_SIGNER_BACKEND='{0}'; expected one of dev / local_file / aws_kms / gcp_kms / phala_tee"
    )]
    UnknownBackend(String),

    /// `SBO3L_SIGNER_BACKEND` named a backend whose Cargo feature was
    /// not enabled at compile time. Rebuild with `--features <name>`.
    #[error("backend '{0}' was not compiled in; rebuild with --features {0}")]
    BackendNotCompiled(&'static str),

    /// A required environment variable is missing or empty (e.g.
    /// `SBO3L_AWS_KMS_KEY_ID` for the AWS backend).
    #[error("environment variable {0} is required for the selected backend")]
    MissingEnv(&'static str),

    /// Wraps an upstream KMS error.
    #[error("KMS request failed: {0}")]
    Kms(String),

    /// The KMS reported a key spec we can't speak (e.g. ECC P-256
    /// instead of Ed25519). Surfaces backend identification mistakes.
    #[error("KMS key '{key_id}' is not Ed25519 (got '{found_spec}')")]
    KeySpecMismatch { key_id: String, found_spec: String },

    /// Hex decoding failed on a signer-internal value.
    #[error("hex decoding: {0}")]
    Hex(#[from] hex::FromHexError),
}

/// Backend-agnostic Ed25519 signer surface used by SBO3L's three signing
/// paths (policy receipt, audit event, decision token).
///
/// Implementations MUST:
/// * Produce 64-byte Ed25519 signatures over the supplied bytes that
///   verify identically against the 32-byte public key reported by
///   [`Signer::verifying_key_hex`].
/// * Report a stable [`Signer::key_id`] that uniquely identifies the
///   key version that produced the signature — verifiers carry this
///   back in `EmbeddedSignature.key_id` and use it for rotation
///   bookkeeping.
/// * Be `Send + Sync` so the daemon can hold them inside `Arc<AppInner>`
///   across tokio tasks.
pub trait Signer: Send + Sync {
    /// Sign `message` and return the 64-byte Ed25519 signature, hex-encoded.
    fn sign_hex(&self, message: &[u8]) -> Result<String, SignerError>;

    /// Return the 32-byte Ed25519 public key, hex-encoded.
    fn verifying_key_hex(&self) -> Result<String, SignerError>;

    /// Stable identifier for the current signing key version.
    fn key_id(&self) -> &str;
}

pub mod dev;
pub mod local_file;

#[cfg(feature = "aws_kms")]
pub mod aws_kms;

#[cfg(feature = "gcp_kms")]
pub mod gcp_kms;

#[cfg(feature = "phala_tee")]
pub mod phala_tee;

#[cfg(feature = "eth_signer")]
pub mod eth;

pub use dev::DevSignerLockedDown;
pub use local_file::{KeyFileFormat, LocalFileSigner};

/// Daemon startup factory. Reads `SBO3L_SIGNER_BACKEND` (default `dev`)
/// and constructs the matching [`Signer`] for the given `role`
/// ("audit", "receipt", "decision"). Backends not compiled into the
/// current binary surface as [`SignerError::BackendNotCompiled`] —
/// callers (the daemon's startup path) typically print the error and
/// exit with code 2.
pub fn signer_from_env(role: &str) -> Result<Box<dyn Signer>, SignerError> {
    let backend = std::env::var("SBO3L_SIGNER_BACKEND").unwrap_or_else(|_| "dev".to_string());
    match backend.as_str() {
        "dev" => Ok(Box::new(DevSignerLockedDown::from_env(role)?)),

        "local_file" => Ok(Box::new(LocalFileSigner::from_env(role)?)),

        "aws_kms" => {
            #[cfg(feature = "aws_kms")]
            {
                Ok(Box::new(aws_kms::AwsKmsSigner::from_env(role)?))
            }
            #[cfg(not(feature = "aws_kms"))]
            {
                let _ = role;
                Err(SignerError::BackendNotCompiled("aws_kms"))
            }
        }

        "gcp_kms" => {
            #[cfg(feature = "gcp_kms")]
            {
                Ok(Box::new(gcp_kms::GcpKmsSigner::from_env(role)?))
            }
            #[cfg(not(feature = "gcp_kms"))]
            {
                let _ = role;
                Err(SignerError::BackendNotCompiled("gcp_kms"))
            }
        }

        "phala_tee" => {
            #[cfg(feature = "phala_tee")]
            {
                Ok(Box::new(phala_tee::PhalaTeeSigner::from_env(role)?))
            }
            #[cfg(not(feature = "phala_tee"))]
            {
                let _ = role;
                Err(SignerError::BackendNotCompiled("phala_tee"))
            }
        }

        other => Err(SignerError::UnknownBackend(other.to_string())),
    }
}
