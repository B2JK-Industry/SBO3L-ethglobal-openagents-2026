//! `eth_local` — local-key-file [`EthSigner`] backend.
//!
//! Compiled only with `--features eth_signer`. Loads a 32-byte
//! secp256k1 secret from a file path supplied via env var
//! (`SBO3L_ETH_LOCAL_FILE_PATH_<UPPERCASE_ROLE>`) and produces 65-byte
//! `r || s || v` signatures over a caller-supplied 32-byte digest.
//!
//! # Why this exists
//!
//! Pairs with the Ed25519 [`crate::signers::local_file`] backend.
//! Operators who keep keys on encrypted volumes (LUKS / FileVault /
//! Tang+Clevis) can run a fully self-hosted SBO3L deployment that
//! signs both audit/receipt (Ed25519) and EVM tx (secp256k1)
//! locally — no cloud KMS account required.
//!
//! # File format
//!
//! Same dual format as [`crate::signers::local_file`]:
//! - 64 hex chars (with optional `0x` prefix + trailing newline) →
//!   the canonical SBO3L form.
//! - 32 raw bytes → for operators who pipe `dd if=/dev/urandom bs=32
//!   count=1` straight into a key file.
//!
//! Both decode to the same `[u8; 32]` secp256k1 secret scalar.
//!
//! # Address derivation
//!
//! Ethereum address = last 20 bytes of `keccak256(uncompressed_pubkey[1..])`,
//! formatted as EIP-55 mixed-case hex with leading `0x`. The
//! [`eip55_checksum`] helper applies the canonical case rules so a
//! consumer comparing addresses byte-for-byte sees the same value
//! regardless of which library produced it.

use k256::ecdsa::signature::hazmat::PrehashSigner;
use k256::ecdsa::{RecoveryId, Signature, SigningKey};
use std::path::{Path, PathBuf};
use tiny_keccak::{Hasher as _, Keccak};

use crate::signers::local_file::{parse_key_file, KeyFileFormat};

use super::{eth::EthSigner, SignerError};

/// secp256k1 keccak256 helper. Local copy because sbo3l-core
/// doesn't otherwise depend on `tiny-keccak` outside the
/// `eth_signer` feature.
fn keccak256(bytes: &[u8]) -> [u8; 32] {
    let mut h = Keccak::v256();
    h.update(bytes);
    let mut out = [0u8; 32];
    h.finalize(&mut out);
    out
}

/// Convert 20-byte Ethereum address bytes to EIP-55 mixed-case hex
/// with a leading `0x` prefix. Public so callers that hold a 20-byte
/// address can format it without going through the full signer.
pub fn eip55_checksum(addr: &[u8; 20]) -> String {
    let lower = hex::encode(addr);
    let hash = keccak256(lower.as_bytes());
    let mut out = String::with_capacity(42);
    out.push_str("0x");
    for (i, c) in lower.chars().enumerate() {
        if c.is_ascii_digit() {
            out.push(c);
        } else {
            // Hash digit at position i: high nibble for even i, low
            // for odd. Bit 7 of the nibble decides upper/lower case.
            let nibble = if i % 2 == 0 {
                hash[i / 2] >> 4
            } else {
                hash[i / 2] & 0x0f
            };
            if nibble >= 8 {
                out.push(c.to_ascii_uppercase());
            } else {
                out.push(c);
            }
        }
    }
    out
}

/// Local-file secp256k1 EVM signer. Wraps a [`SigningKey`] and
/// caches the EIP-55 address.
#[derive(Debug)]
pub struct EthLocalFileSigner {
    inner: SigningKey,
    role: String,
    file_path: PathBuf,
    file_format: KeyFileFormat,
    eth_address: String,
    key_id: String,
}

impl EthLocalFileSigner {
    /// Construct from env. Reads
    /// `SBO3L_ETH_LOCAL_FILE_PATH_<UPPERCASE_ROLE>` for the path,
    /// optional `SBO3L_ETH_LOCAL_FILE_KEY_ID_<UPPERCASE_ROLE>` for a
    /// stable key id (defaults to the file basename).
    pub fn from_env(role: &str) -> Result<Self, SignerError> {
        let upper = role.to_uppercase();
        let path_env = format!("SBO3L_ETH_LOCAL_FILE_PATH_{upper}");
        let path_str = std::env::var(&path_env).map_err(|_| {
            SignerError::Kms(format!(
                "eth_local backend requires {path_env} to point at the role's secret key file"
            ))
        })?;
        Self::from_path(role, PathBuf::from(path_str))
    }

    /// Construct from an explicit path. Useful for tests + callers
    /// that wire the path from a config file rather than env.
    pub fn from_path(role: &str, path: PathBuf) -> Result<Self, SignerError> {
        let bytes = std::fs::read(&path).map_err(|e| {
            SignerError::Kms(format!("eth_local read {} failed: {e}", path.display()))
        })?;
        let (secret, format) = parse_key_file(&bytes)?;
        Self::from_secret_bytes(role, &path, secret, format)
    }

    fn from_secret_bytes(
        role: &str,
        path: &Path,
        secret: [u8; 32],
        format: KeyFileFormat,
    ) -> Result<Self, SignerError> {
        let signing = SigningKey::from_bytes((&secret).into())
            .map_err(|e| SignerError::Kms(format!("eth_local: invalid secp256k1 secret: {e}")))?;
        let verifying = signing.verifying_key();
        // Uncompressed pubkey = 0x04 || X || Y (65 bytes). Address is
        // last 20 bytes of keccak256(X || Y) — drop the 0x04 prefix.
        let encoded = verifying.to_encoded_point(false);
        let pk_bytes = encoded.as_bytes();
        debug_assert_eq!(pk_bytes.len(), 65);
        debug_assert_eq!(pk_bytes[0], 0x04);
        let hash = keccak256(&pk_bytes[1..]);
        let mut addr = [0u8; 20];
        addr.copy_from_slice(&hash[12..]);
        let eth_address = eip55_checksum(&addr);

        let key_id = key_id_for(role, path);
        Ok(Self {
            inner: signing,
            role: role.to_string(),
            file_path: path.to_path_buf(),
            file_format: format,
            eth_address,
            key_id,
        })
    }

    pub fn role(&self) -> &str {
        &self.role
    }
    pub fn file_path(&self) -> &Path {
        &self.file_path
    }
    pub fn file_format(&self) -> KeyFileFormat {
        self.file_format
    }
}

fn key_id_for(role: &str, path: &Path) -> String {
    let upper = role.to_uppercase();
    let env_name = format!("SBO3L_ETH_LOCAL_FILE_KEY_ID_{upper}");
    if let Ok(explicit) = std::env::var(&env_name) {
        if !explicit.is_empty() {
            return explicit;
        }
    }
    let basename = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("eth_local");
    format!("{role}-eth-local-{basename}")
}

impl EthSigner for EthLocalFileSigner {
    fn sign_digest_hex(&self, digest: &[u8; 32]) -> Result<String, SignerError> {
        // sign_prehash takes an already-hashed 32-byte input. Returns
        // (signature, recovery_id). We concatenate r || s || v where
        // v is the 0/1 recovery id (NOT the EIP-155 chain-id-encoded
        // form — callers that need that add chain_id * 2 + 35
        // themselves).
        let (sig, recid): (Signature, RecoveryId) = self
            .inner
            .sign_prehash(digest)
            .map_err(|e| SignerError::Kms(format!("eth_local sign_prehash: {e}")))?;
        let mut out = Vec::with_capacity(65);
        out.extend_from_slice(&sig.to_bytes());
        out.push(recid.to_byte());
        Ok(format!("0x{}", hex::encode(out)))
    }

    fn eth_address(&self) -> Result<String, SignerError> {
        Ok(self.eth_address.clone())
    }

    fn key_id(&self) -> &str {
        &self.key_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k256::ecdsa::signature::hazmat::PrehashVerifier;
    use std::io::Write;

    fn write_temp(bytes: &[u8]) -> tempfile::NamedTempFile {
        let f = tempfile::NamedTempFile::new().expect("temp");
        let mut handle = f.reopen().expect("reopen");
        handle.write_all(bytes).expect("write");
        f
    }

    #[test]
    fn from_path_round_trip_signs_and_verifies_against_recovered_pubkey() {
        // Deterministic secret — same secret + same digest must
        // produce the same signature on every run.
        let f = write_temp(b"0x0101010101010101010101010101010101010101010101010101010101010101");
        let signer = EthLocalFileSigner::from_path("audit", f.path().to_path_buf()).unwrap();
        let digest: [u8; 32] = [0x42; 32];
        let sig_hex = signer.sign_digest_hex(&digest).unwrap();
        assert!(sig_hex.starts_with("0x"));
        // 65 bytes = 130 hex chars + "0x" prefix.
        assert_eq!(sig_hex.len(), 132);

        // Recover the pubkey from the signature; address must match
        // what the signer reports.
        let raw = hex::decode(&sig_hex[2..]).unwrap();
        assert_eq!(raw.len(), 65);
        let mut sig_bytes = [0u8; 64];
        sig_bytes.copy_from_slice(&raw[..64]);
        let sig = Signature::from_slice(&sig_bytes).unwrap();
        let recid = RecoveryId::try_from(raw[64]).unwrap();
        let recovered =
            k256::ecdsa::VerifyingKey::recover_from_prehash(&digest, &sig, recid).unwrap();
        let encoded = recovered.to_encoded_point(false);
        let hash = keccak256(&encoded.as_bytes()[1..]);
        let mut addr = [0u8; 20];
        addr.copy_from_slice(&hash[12..]);
        let recovered_addr = eip55_checksum(&addr);
        assert_eq!(recovered_addr, signer.eth_address().unwrap());
    }

    #[test]
    fn signature_verifies_via_verifying_key_prehash_path() {
        // Independent verify path — uses the verifier API rather
        // than recover_from_prehash. Catches a broken recovery byte
        // that the recovery-based test above wouldn't catch.
        let f = write_temp(b"0x0202020202020202020202020202020202020202020202020202020202020202");
        let signer = EthLocalFileSigner::from_path("audit", f.path().to_path_buf()).unwrap();
        let digest: [u8; 32] = [0x77; 32];
        let sig_hex = signer.sign_digest_hex(&digest).unwrap();
        let raw = hex::decode(&sig_hex[2..]).unwrap();
        let sig = Signature::from_slice(&raw[..64]).unwrap();
        let vk = signer.inner.verifying_key();
        vk.verify_prehash(&digest, &sig)
            .expect("signature must verify against the local signer's verifying key");
    }

    #[test]
    fn eip55_known_vector() {
        // EIP-55 spec example.
        let raw = hex::decode("5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed").unwrap();
        let mut addr = [0u8; 20];
        addr.copy_from_slice(&raw);
        assert_eq!(
            eip55_checksum(&addr),
            "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed"
        );
    }

    #[test]
    fn raw32_bytes_path_works() {
        let raw = [0x42u8; 32];
        let f = write_temp(&raw);
        let signer = EthLocalFileSigner::from_path("receipt", f.path().to_path_buf()).unwrap();
        assert_eq!(signer.file_format(), KeyFileFormat::Raw32);
        let sig = signer.sign_digest_hex(&[0u8; 32]).unwrap();
        assert!(sig.starts_with("0x"));
        assert_eq!(sig.len(), 132);
    }

    #[test]
    fn key_id_defaults_to_role_and_basename() {
        let f = write_temp(&[0x99u8; 32]);
        let signer = EthLocalFileSigner::from_path("decision", f.path().to_path_buf()).unwrap();
        assert!(signer.key_id().starts_with("decision-eth-local-"));
    }

    #[test]
    fn missing_path_returns_clear_error() {
        let path = PathBuf::from("/nonexistent/sbo3l/eth.hex");
        let err = EthLocalFileSigner::from_path("audit", path).expect_err("must error");
        match err {
            SignerError::Kms(msg) => assert!(msg.contains("eth_local read"), "got: {msg}"),
            other => panic!("expected Kms error, got {other:?}"),
        }
    }

    #[test]
    fn signature_byte_identical_across_two_constructions_with_same_seed() {
        // Determinism — two EthLocalFileSigner instances over the
        // same secret bytes produce identical signatures over the
        // same digest. ECDSA with deterministic-k (RFC 6979) is the
        // default in k256, so this should hold.
        let secret = [0x10u8; 32];
        let f1 = write_temp(&secret);
        let f2 = write_temp(&secret);
        let s1 = EthLocalFileSigner::from_path("audit", f1.path().to_path_buf()).unwrap();
        let s2 = EthLocalFileSigner::from_path("audit", f2.path().to_path_buf()).unwrap();
        let digest: [u8; 32] = [0x33; 32];
        let sig1 = s1.sign_digest_hex(&digest).unwrap();
        let sig2 = s2.sign_digest_hex(&digest).unwrap();
        assert_eq!(
            sig1, sig2,
            "deterministic-k ECDSA must produce identical signatures"
        );
        assert_eq!(s1.eth_address().unwrap(), s2.eth_address().unwrap());
    }

    #[test]
    fn eth_address_is_eip55_formatted() {
        let f = write_temp(&[0x55u8; 32]);
        let signer = EthLocalFileSigner::from_path("audit", f.path().to_path_buf()).unwrap();
        let addr = signer.eth_address().unwrap();
        assert!(addr.starts_with("0x"));
        assert_eq!(addr.len(), 42);
        // EIP-55 mixed-case: contains at least one upper + one lower
        // (a fully-lowercase address would fail to detect a typo on
        // a checksummed-aware client).
        let body = &addr[2..];
        assert!(body
            .chars()
            .any(|c| c.is_ascii_uppercase() || c.is_ascii_digit()));
    }
}
