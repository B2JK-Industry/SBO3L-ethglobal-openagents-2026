//! `eth_kms` — cloud-KMS [`EthSigner`] backends.
//!
//! Compiled only with `--features eth_signer`. Two sub-backends, each
//! gated behind its own KMS feature:
//!
//! - `aws::AwsEthKmsSigner` (`--features aws_kms`)
//! - `gcp::GcpEthKmsSigner` (`--features gcp_kms`)
//!
//! Both follow the same shape as the Ed25519 KMS stubs (see
//! `super::aws_kms` and `super::gcp_kms`): the trait + factory plumb
//! lands here today; the real SDK round-trips (`aws-sdk-kms`,
//! `google-cloud-kms`) land in a follow-up that pulls those crates
//! behind the matching Cargo feature. Keeping the SDKs out of the
//! default build saves ~150 transitive crates.
//!
//! # Why eth-side KMS at all?
//!
//! AWS KMS supports `ECC_SECG_P256K1` key specs (the curve EVM uses).
//! GCP KMS supports `EC_SIGN_SECP256K1_SHA256`. A production
//! deployment that already runs Ed25519 audit/receipt signing under
//! KMS gets one consistent place to manage key rotation, IAM, audit
//! trails — versus mixing local-file EVM keys with cloud-KMS
//! Ed25519 keys, which is operationally awkward.
//!
//! # Sign + recovery shape
//!
//! Both KMS APIs return DER-encoded ECDSA signatures (ASN.1
//! `Sig ::= SEQUENCE { r INTEGER, s INTEGER }`). The trait's
//! `sign_digest_hex` returns 65-byte `r || s || v` — the future
//! impls will:
//!
//! 1. Call the KMS Sign API with the supplied 32-byte digest.
//! 2. Parse DER → `(r, s)`, normalize `s` to low-S form (EIP-2),
//!    re-pack as 64-byte concat.
//! 3. Compute `v` (recovery id) by trying both 0 and 1, recovering
//!    the public key, and matching against the cached pubkey.
//!
//! Step 3 is the only EVM-specific subtlety vs the Ed25519 KMS
//! shape. Documented here so the follow-up implementer doesn't
//! re-discover it.

#[cfg(feature = "aws_kms")]
pub mod aws {
    use crate::signers::{eth::EthSigner, SignerError};

    /// Stub — actual SDK call lands in the AWS KMS feature follow-up.
    pub struct AwsEthKmsSigner {
        key_id: String,
    }

    impl AwsEthKmsSigner {
        pub fn from_env(_role: &str) -> Result<Self, SignerError> {
            let key_id = std::env::var("SBO3L_ETH_AWS_KMS_KEY_ID")
                .map_err(|_| SignerError::MissingEnv("SBO3L_ETH_AWS_KMS_KEY_ID"))?;
            if key_id.is_empty() {
                return Err(SignerError::MissingEnv("SBO3L_ETH_AWS_KMS_KEY_ID"));
            }
            Ok(Self { key_id })
        }
    }

    impl EthSigner for AwsEthKmsSigner {
        fn sign_digest_hex(&self, _digest: &[u8; 32]) -> Result<String, SignerError> {
            Err(SignerError::Kms(format!(
                "aws_kms eth signer (key={}) not yet implemented; pull aws-sdk-kms in the eth_kms_aws_live follow-up",
                self.key_id
            )))
        }

        fn eth_address(&self) -> Result<String, SignerError> {
            Err(SignerError::Kms(format!(
                "aws_kms eth signer (key={}) not yet implemented",
                self.key_id
            )))
        }

        fn key_id(&self) -> &str {
            &self.key_id
        }
    }
}

#[cfg(feature = "gcp_kms")]
pub mod gcp {
    use crate::signers::{eth::EthSigner, SignerError};

    /// Stub — actual SDK call lands in the GCP KMS feature follow-up.
    pub struct GcpEthKmsSigner {
        key_name: String,
    }

    impl GcpEthKmsSigner {
        pub fn from_env(_role: &str) -> Result<Self, SignerError> {
            let key_name = std::env::var("SBO3L_ETH_GCP_KMS_KEY_NAME")
                .map_err(|_| SignerError::MissingEnv("SBO3L_ETH_GCP_KMS_KEY_NAME"))?;
            if key_name.is_empty() {
                return Err(SignerError::MissingEnv("SBO3L_ETH_GCP_KMS_KEY_NAME"));
            }
            Ok(Self { key_name })
        }
    }

    impl EthSigner for GcpEthKmsSigner {
        fn sign_digest_hex(&self, _digest: &[u8; 32]) -> Result<String, SignerError> {
            Err(SignerError::Kms(format!(
                "gcp_kms eth signer (key={}) not yet implemented; pull google-cloud-kms in the eth_kms_gcp_live follow-up",
                self.key_name
            )))
        }

        fn eth_address(&self) -> Result<String, SignerError> {
            Err(SignerError::Kms(format!(
                "gcp_kms eth signer (key={}) not yet implemented",
                self.key_name
            )))
        }

        fn key_id(&self) -> &str {
            &self.key_name
        }
    }
}
