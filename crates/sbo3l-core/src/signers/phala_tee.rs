//! `phala_tee` backend (feature `phala_tee`) — Phase 3 placeholder.
//!
//! Phala Network's TEE-attested signing service holds the private key
//! inside an Intel SGX or NVIDIA H100-confidential-VM enclave. Each
//! signature comes back with an attestation that the signing was
//! performed inside the enclave with code matching a published
//! measurement. Pair with a verifier that checks the attestation root
//! before trusting the signature → "this signature came from a known
//! piece of code running in a tamper-resistant environment".
//!
//! # Status — F-5 hackathon scope
//!
//! Compile-only stub. **Not a real TEE today.** Phase 3 wires the
//! actual `dstack` / Phala remote-attestation flow; this module exists
//! to lock the trait shape and the env var name (`SBO3L_PHALA_TEE_KEY_ID`)
//! so the rest of the codebase can plumb through a `Box<dyn Signer>`
//! without churning when the real backend lands.

use super::{Signer, SignerError};

pub struct PhalaTeeSigner {
    key_id: String,
}

impl PhalaTeeSigner {
    pub fn from_env(_role: &str) -> Result<Self, SignerError> {
        let key_id = std::env::var("SBO3L_PHALA_TEE_KEY_ID")
            .map_err(|_| SignerError::MissingEnv("SBO3L_PHALA_TEE_KEY_ID"))?;
        if key_id.is_empty() {
            return Err(SignerError::MissingEnv("SBO3L_PHALA_TEE_KEY_ID"));
        }
        Ok(Self { key_id })
    }
}

impl Signer for PhalaTeeSigner {
    fn sign_hex(&self, _message: &[u8]) -> Result<String, SignerError> {
        Err(SignerError::Kms(format!(
            "phala_tee backend ({}) is a Phase 3 placeholder; real TEE wiring not in this build",
            self.key_id
        )))
    }

    fn verifying_key_hex(&self) -> Result<String, SignerError> {
        Err(SignerError::Kms(format!(
            "phala_tee backend ({}) is a Phase 3 placeholder; real TEE wiring not in this build",
            self.key_id
        )))
    }

    fn key_id(&self) -> &str {
        &self.key_id
    }
}
