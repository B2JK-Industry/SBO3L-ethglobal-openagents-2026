//! `dev` backend — local Ed25519 signer with a production-mode lockout.
//!
//! The dev signer uses the same [`crate::signer::DevSigner`] code path the
//! rest of the codebase already exercises (demo gates, integration tests,
//! `MockKmsSigner` fixtures), but its constructor refuses unless
//! `SBO3L_DEV_ONLY_SIGNER=1` is explicitly set. This shifts the
//! "is this dev or prod" question from "we couldn't tell, the seeds are
//! public" (pre-F-5) to "the operator has affirmatively asked for dev
//! mode" (post-F-5). Pair with the F-1
//! `⚠ UNAUTHENTICATED MODE — DEV ONLY ⚠` banner: a daemon running with
//! both flags is loud about every dev-mode shortcut it's taking.

use crate::signer::DevSigner;

use super::{Signer, SignerError};

/// Per-role deterministic dev seeds. Only used when
/// `SBO3L_DEV_ONLY_SIGNER=1` is set; the seeds are public constants in
/// this repo (anyone can forge a signature that passes `verify_hex`)
/// and the wire format is identical across any production replacement,
/// so swapping in a real backend changes the trust root without
/// changing call sites.
fn seed_for_role(role: &str) -> [u8; 32] {
    match role {
        "audit" => [11u8; 32],
        "receipt" | "decision" => [7u8; 32],
        _ => {
            // Distinct seed per unrecognised role so two different
            // callers don't accidentally share a key just because they
            // both fell through to the default.
            let mut s = [0u8; 32];
            for (i, b) in role.as_bytes().iter().enumerate() {
                s[i % 32] ^= *b;
            }
            s
        }
    }
}

/// Wrapper around [`DevSigner`] that gates construction on the
/// `SBO3L_DEV_ONLY_SIGNER=1` env var. Any code path that goes through
/// [`crate::signers::signer_from_env`] sees the lockout. Tests and
/// demos that hold a `DevSigner` directly bypass it (intended — they
/// run in known-dev contexts).
pub struct DevSignerLockedDown {
    inner: DevSigner,
    role: String,
}

impl DevSignerLockedDown {
    /// Construct from env. Returns [`SignerError::DevOnlyLockout`] if
    /// `SBO3L_DEV_ONLY_SIGNER` is unset or any value other than `"1"`.
    /// On success, prints a `⚠ DEV ONLY SIGNER ⚠` banner to stderr —
    /// the QA test plan greps for this exact substring.
    pub fn from_env(role: &str) -> Result<Self, SignerError> {
        if std::env::var("SBO3L_DEV_ONLY_SIGNER").as_deref() != Ok("1") {
            return Err(SignerError::DevOnlyLockout);
        }
        eprintln!("⚠ DEV ONLY SIGNER ⚠");
        eprintln!(
            "  SBO3L_DEV_ONLY_SIGNER=1 is set; using deterministic public dev seeds for role '{role}'."
        );
        eprintln!(
            "  Anyone with this repo can forge signatures that pass verify_hex against this backend."
        );
        let key_id = format!("{role}-dev-v1");
        Ok(Self {
            inner: DevSigner::from_seed(key_id, seed_for_role(role)),
            role: role.to_string(),
        })
    }

    /// Direct accessor for the inner [`DevSigner`]. Used only by the
    /// daemon's startup path so it can keep its existing
    /// `audit_signer: DevSigner` field shape; production paths route
    /// through the [`Signer`] trait.
    pub fn inner(&self) -> &DevSigner {
        &self.inner
    }

    /// Move the inner [`DevSigner`] out for callers that need the
    /// concrete type (e.g. the existing `AppState::with_signers`
    /// constructor).
    pub fn into_inner(self) -> DevSigner {
        self.inner
    }

    pub fn role(&self) -> &str {
        &self.role
    }
}

impl Signer for DevSignerLockedDown {
    fn sign_hex(&self, message: &[u8]) -> Result<String, SignerError> {
        Ok(self.inner.sign_hex(message))
    }

    fn verifying_key_hex(&self) -> Result<String, SignerError> {
        Ok(self.inner.verifying_key_hex())
    }

    fn key_id(&self) -> &str {
        &self.inner.key_id
    }
}

/// Blanket [`Signer`] impl for the bare [`DevSigner`] — lets tests and
/// the audit/receipt code paths that already hold a concrete `DevSigner`
/// participate in the new trait without going through the env-gated
/// wrapper.
impl Signer for DevSigner {
    fn sign_hex(&self, message: &[u8]) -> Result<String, SignerError> {
        Ok(DevSigner::sign_hex(self, message))
    }

    fn verifying_key_hex(&self) -> Result<String, SignerError> {
        Ok(DevSigner::verifying_key_hex(self))
    }

    fn key_id(&self) -> &str {
        &self.key_id
    }
}
