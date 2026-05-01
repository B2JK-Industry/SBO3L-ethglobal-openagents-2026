//! `local_file` signer backend — reads an Ed25519 secret key from a
//! file path supplied via env var.
//!
//! The intermediate step between the env-only `dev` backend (which
//! uses public deterministic seeds) and the cloud KMS backends
//! (`aws_kms`, `gcp_kms`). Use cases:
//!
//! - **Air-gapped operators** who keep keys on a hardware-backed
//!   encrypted volume (LUKS, FileVault, Tang/Clevis, YubiKey-protected
//!   PIV) and don't want to reach a cloud KMS.
//! - **Self-hosted deployments** without an aws/gcp account.
//! - **CI rotation drills** where the secret rotates per pipeline
//!   run; pointing the daemon at a fresh file is faster than
//!   reconfiguring AWS access.
//! - **Reproducible test fixtures** — pin a specific Ed25519 secret
//!   in a temp file and walk a multi-call signing scenario through
//!   a deterministic key.
//!
//! # Wire shape
//!
//! Two file formats are accepted; the loader detects by content:
//!
//! 1. **Hex** — file is exactly 64 hex characters (optionally with a
//!    `0x` prefix and trailing newline), encoding a 32-byte Ed25519
//!    secret seed. This is the canonical SBO3L format.
//! 2. **Raw 32 bytes** — file is exactly 32 bytes. Lets operators
//!    pipe `dd if=/dev/urandom bs=32 count=1` straight into a key
//!    file without a hex layer.
//!
//! Anything else returns [`SignerError::Kms`] with a clear "expected
//! 64 hex chars or 32 raw bytes" message.
//!
//! # Env vars
//!
//! - `SBO3L_LOCAL_FILE_PATH_<UPPERCASE_ROLE>` — path to the secret
//!   for the named role (`audit`, `receipt`, `decision`). The role
//!   suffix mirrors the dev backend's per-role key separation; an
//!   operator who wants the same key across all roles symlinks the
//!   files.
//! - `SBO3L_LOCAL_FILE_KEY_ID_<UPPERCASE_ROLE>` — optional stable
//!   identifier for [`Signer::key_id`]. Defaults to the file's
//!   basename if unset.
//!
//! # Permission posture
//!
//! The loader does NOT enforce file mode 0600 — operators who run
//! SBO3L under a custom umask or store keys on filesystems that
//! don't honour POSIX permissions (FAT-32 USB sticks, network
//! shares) would be locked out for no security gain. Production
//! deployments should pair this backend with an external check
//! (`stat -c %a`, AppArmor profile, etc.).

use std::path::{Path, PathBuf};

use crate::signer::DevSigner;

use super::{Signer, SignerError};

/// Result of parsing a key file. `kind` is exposed in test asserts +
/// the [`Signer::key_id`] suffix when no explicit override was given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyFileFormat {
    Hex64,
    Raw32,
}

/// Parse a key file's bytes. Public so tests + the loader share one
/// detection path.
pub fn parse_key_file(bytes: &[u8]) -> Result<([u8; 32], KeyFileFormat), SignerError> {
    // Try hex first — strip whitespace, optional 0x prefix.
    let trimmed: Vec<u8> = bytes
        .iter()
        .copied()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    let trimmed_slice: &[u8] = if trimmed.starts_with(b"0x") || trimmed.starts_with(b"0X") {
        &trimmed[2..]
    } else {
        &trimmed
    };
    if trimmed_slice.len() == 64 && trimmed_slice.iter().all(|b| b.is_ascii_hexdigit()) {
        let decoded = hex::decode(trimmed_slice)?;
        let arr: [u8; 32] = decoded
            .try_into()
            .expect("64 hex chars decode to exactly 32 bytes");
        return Ok((arr, KeyFileFormat::Hex64));
    }
    // Fall back to raw 32-byte case.
    if bytes.len() == 32 {
        let arr: [u8; 32] = bytes
            .try_into()
            .expect("len 32 array conversion infallible");
        return Ok((arr, KeyFileFormat::Raw32));
    }
    Err(SignerError::Kms(format!(
        "key file must be 64 hex chars or 32 raw bytes; got {} bytes (post-trim {} chars)",
        bytes.len(),
        trimmed_slice.len()
    )))
}

/// Local-file Ed25519 signer. Wraps [`DevSigner`] internally so the
/// signing path is byte-identical to the rest of the codebase — a
/// receipt signed here verifies under the same `verify_hex` as one
/// from any other backend.
#[derive(Debug)]
pub struct LocalFileSigner {
    inner: DevSigner,
    role: String,
    file_path: PathBuf,
    file_format: KeyFileFormat,
}

impl LocalFileSigner {
    /// Construct from env. Reads
    /// `SBO3L_LOCAL_FILE_PATH_<UPPERCASE_ROLE>` for the path, optional
    /// `SBO3L_LOCAL_FILE_KEY_ID_<UPPERCASE_ROLE>` for a stable key id.
    pub fn from_env(role: &str) -> Result<Self, SignerError> {
        let upper = role.to_uppercase();
        let path_env = format!("SBO3L_LOCAL_FILE_PATH_{upper}");
        let path_str = std::env::var(&path_env).map_err(|_| {
            // The MissingEnv variant requires &'static; the role
            // suffix is dynamic, so we route through Kms with a
            // clear "missing env" message instead of leaking.
            SignerError::Kms(format!(
                "local_file backend requires {path_env} to point at the role's secret key file"
            ))
        })?;
        Self::from_path(role, PathBuf::from(path_str))
    }

    /// Construct from an explicit path. Useful for tests + callers
    /// that want to wire the path from a config file rather than env.
    pub fn from_path(role: &str, path: PathBuf) -> Result<Self, SignerError> {
        let bytes = std::fs::read(&path).map_err(|e| {
            SignerError::Kms(format!("local_file read {} failed: {e}", path.display()))
        })?;
        let (seed, format) = parse_key_file(&bytes)?;
        let key_id = key_id_for(role, &path);
        Ok(Self {
            inner: DevSigner::from_seed(key_id, seed),
            role: role.to_string(),
            file_path: path,
            file_format: format,
        })
    }

    pub fn role(&self) -> &str {
        &self.role
    }
    pub fn file_path(&self) -> &PathBuf {
        &self.file_path
    }
    pub fn file_format(&self) -> KeyFileFormat {
        self.file_format
    }
}

fn key_id_for(role: &str, path: &Path) -> String {
    let upper = role.to_uppercase();
    let env_name = format!("SBO3L_LOCAL_FILE_KEY_ID_{upper}");
    if let Ok(explicit) = std::env::var(&env_name) {
        if !explicit.is_empty() {
            return explicit;
        }
    }
    let basename = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("local_file");
    format!("{role}-local-file-{basename}")
}

impl Signer for LocalFileSigner {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signer::verify_hex;
    use std::io::Write;

    fn write_temp(bytes: &[u8]) -> tempfile::NamedTempFile {
        let f = tempfile::NamedTempFile::new().expect("temp");
        let mut handle = f.reopen().expect("reopen");
        handle.write_all(bytes).expect("write");
        f
    }

    #[test]
    fn parse_hex64_with_prefix_and_newline() {
        let mut bytes = Vec::from(b"0x".as_slice());
        bytes.extend_from_slice(&[b'a'; 64]);
        bytes.push(b'\n');
        let (arr, fmt) = parse_key_file(&bytes).unwrap();
        assert_eq!(arr, [0xaa; 32]);
        assert_eq!(fmt, KeyFileFormat::Hex64);
    }

    #[test]
    fn parse_hex64_without_prefix() {
        let bytes = vec![b'b'; 64];
        let (arr, fmt) = parse_key_file(&bytes).unwrap();
        assert_eq!(arr, [0xbb; 32]);
        assert_eq!(fmt, KeyFileFormat::Hex64);
    }

    #[test]
    fn parse_raw32_bytes() {
        let bytes = [0x42u8; 32];
        let (arr, fmt) = parse_key_file(&bytes).unwrap();
        assert_eq!(arr, [0x42; 32]);
        assert_eq!(fmt, KeyFileFormat::Raw32);
    }

    #[test]
    fn parse_rejects_short_input() {
        let bytes = [0x42u8; 16];
        assert!(parse_key_file(&bytes).is_err());
    }

    #[test]
    fn parse_rejects_invalid_hex() {
        // 64 chars but `g` not hex
        let bytes = vec![b'g'; 64];
        assert!(parse_key_file(&bytes).is_err());
    }

    #[test]
    fn from_path_round_trip_signs_and_verifies() {
        let f = write_temp(b"0x0101010101010101010101010101010101010101010101010101010101010101");
        let signer = LocalFileSigner::from_path("audit", f.path().to_path_buf()).unwrap();
        let msg = b"hello world";
        let sig_hex = signer.sign_hex(msg).unwrap();
        let pk_hex = signer.verifying_key_hex().unwrap();
        // Verifies via the canonical verify_hex — proves wire-format
        // compatibility with every other backend.
        assert!(verify_hex(&pk_hex, msg, &sig_hex).is_ok());
        assert_eq!(signer.role(), "audit");
        assert_eq!(signer.file_format(), KeyFileFormat::Hex64);
    }

    #[test]
    fn from_path_raw32_works() {
        let raw = [0x77u8; 32];
        let f = write_temp(&raw);
        let signer = LocalFileSigner::from_path("receipt", f.path().to_path_buf()).unwrap();
        assert_eq!(signer.file_format(), KeyFileFormat::Raw32);
        let sig = signer.sign_hex(b"x").unwrap();
        let pk = signer.verifying_key_hex().unwrap();
        assert!(verify_hex(&pk, b"x", &sig).is_ok());
    }

    #[test]
    fn key_id_defaults_to_role_and_basename() {
        let f = write_temp(&[0x99u8; 32]);
        let signer = LocalFileSigner::from_path("decision", f.path().to_path_buf()).unwrap();
        let id = signer.key_id();
        assert!(id.starts_with("decision-local-file-"));
    }

    #[test]
    fn missing_path_returns_clear_error() {
        let path = PathBuf::from("/nonexistent/sbo3l/key.hex");
        let err = LocalFileSigner::from_path("audit", path).expect_err("must error");
        match err {
            SignerError::Kms(msg) => {
                assert!(msg.contains("local_file read"), "got: {msg}");
            }
            other => panic!("expected Kms error, got {other:?}"),
        }
    }

    #[test]
    fn signature_byte_identical_across_two_constructions_with_same_seed() {
        // Two LocalFileSigner instances pointing at the same content
        // produce identical signatures over the same message —
        // confirms determinism + that the wire format doesn't drift
        // across daemon restarts.
        let f1 = write_temp(&[0x10u8; 32]);
        let f2 = write_temp(&[0x10u8; 32]);
        let s1 = LocalFileSigner::from_path("audit", f1.path().to_path_buf()).unwrap();
        let s2 = LocalFileSigner::from_path("audit", f2.path().to_path_buf()).unwrap();
        let msg = b"the same message";
        assert_eq!(s1.sign_hex(msg).unwrap(), s2.sign_hex(msg).unwrap());
        assert_eq!(
            s1.verifying_key_hex().unwrap(),
            s2.verifying_key_hex().unwrap()
        );
    }
}
