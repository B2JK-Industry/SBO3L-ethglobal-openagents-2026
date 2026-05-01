//! `eth_signer` feature — sibling secp256k1 trait for EVM transactions.
//!
//! # Why a sibling trait, not [`super::Signer`] generalised
//!
//! [`super::Signer`] is **Ed25519** (curve: edwards25519, signature:
//! 64 bytes, no recovery id). EVM transactions sign **secp256k1**
//! digests (signature: 65 bytes = `r || s || v`, where `v` is the
//! recovery id). The wire formats are different sizes, the verifying
//! shape is different (32-byte Ed25519 pubkey vs 20-byte Ethereum
//! address derived from the secp256k1 public key), and the address
//! hashing rules differ (raw key bytes vs Keccak256 + last 20 bytes).
//!
//! Forcing both into one trait would require either an enum return
//! type or a "what kind of signature do you want" generic parameter
//! at every call site — both leak the shape difference into every
//! caller. Two sibling traits are clearer.
//!
//! Both backends MAY be hosted in the same KMS (AWS KMS supports both
//! Ed25519 and secp256k1 key specs; GCP KMS supports both); a
//! deployment-time decision picks which key the EVM signer uses.
//!
//! # Status — F-5 ships the trait stub only
//!
//! Dev 1 (Rust Core) lands the trait shape so the rest of the code
//! base can plumb `Box<dyn EthSigner>` through configs and adapters
//! without breaking when the real impl lands.
//!
//! Dev 4 (Infra + On-chain) wires the actual EVM backend in T-3-1
//! (Durin subname issuance). Implementation will mirror the AWS KMS
//! Ed25519 backend in shape — `from_env` constructor reading
//! `SBO3L_ETH_SIGNER_BACKEND` + per-backend env vars, sync `sign_*`
//! shimmed over the async SDK via `block_in_place`.

use super::SignerError;

/// secp256k1 EVM transaction signer. Sibling trait to [`super::Signer`].
///
/// Implementations MUST:
///
/// * Produce 65-byte secp256k1 signatures (`r || s || v` where `v` is
///   the recovery id, 0 or 1) over a 32-byte digest. The digest is
///   already-hashed (e.g. EIP-191 personal-sign digest, or EIP-712
///   typed-data digest, or a raw transaction `keccak256` digest);
///   implementations DO NOT re-hash.
/// * Report the 20-byte Ethereum address derived from the public key
///   in EIP-55 mixed-case hex (with leading `0x`).
/// * Be `Send + Sync`.
pub trait EthSigner: Send + Sync {
    /// Sign a 32-byte digest. Returns the 65-byte secp256k1 signature
    /// (`r || s || v`), hex-encoded with leading `0x`.
    fn sign_digest_hex(&self, digest: &[u8; 32]) -> Result<String, SignerError>;

    /// Ethereum address derived from the verifying key, EIP-55
    /// mixed-case hex with leading `0x`. Stable across signatures.
    fn eth_address(&self) -> Result<String, SignerError>;

    /// Stable identifier for the current key version (mirrors
    /// [`super::Signer::key_id`]).
    fn key_id(&self) -> &str;
}
