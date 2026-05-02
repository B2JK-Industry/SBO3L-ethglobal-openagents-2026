//! Audit-root digest builder.
//!
//! Computes a single 32-byte commitment to an SBO3L audit chain
//! prefix. Two designs were considered:
//!
//! - **Merkle root** of every event hash. O(n) work to build, O(log n)
//!   to prove a single event's inclusion. The natural shape if we
//!   also wanted on-chain verification of individual events.
//! - **Tip-hash chaining** — `audit_root = keccak256(prev_root ||
//!   chain_head_event_hash || chain_head_seq)`. O(1) per anchor,
//!   trivially verifiable by reading the chain to its tip.
//!
//! Phase 3.1 uses **chain-head hashing**. The on-chain anchor's job
//! is "this seq → this event_hash existed at this timestamp"; the
//! cryptographic chain inside SBO3L already provides the prefix
//! integrity guarantee, so we just need a stable summary of the
//! tip. A future Phase 3.2 can add a Merkle root in parallel for
//! single-event inclusion proofs.
//!
//! # Wire shape
//!
//! `audit_root = keccak256("sbo3l.audit_root.v1" || network || head_seq_be8 || head_event_hash_32)`
//!
//! - The `"sbo3l.audit_root.v1"` ASCII tag domain-separates this
//!   digest from any other keccak256 hash in the codebase
//!   (defends against cross-protocol replay).
//! - `network` is the lowercase ASCII network name
//!   (`"mainnet"` / `"sepolia"`); same audit chain anchored on
//!   different networks produces different roots so a reader
//!   knows which they're verifying against.
//! - `head_seq_be8` is the 8-byte big-endian u64 chain-head seq.
//! - `head_event_hash_32` is the raw 32-byte tip event hash.
//!
//! Output is the 32-byte keccak256 digest, hex-encoded with
//! leading `0x`.

use thiserror::Error;
use tiny_keccak::{Hasher as _, Keccak};

// PartialEq/Eq omitted — `hex::FromHexError` doesn't impl Eq, and
// callers should compare on shape (matches!) rather than equality.
#[derive(Debug, Error)]
pub enum AuditRootError {
    #[error("audit_root: empty audit chain (chain_head_event_hash must be 64 hex chars)")]
    EmptyChain,
    #[error("audit_root: chain_head_event_hash must be 64-char hex (no `0x`); got {0}")]
    BadEventHashShape(String),
    #[error("audit_root: hex decode failed: {0}")]
    Hex(#[from] hex::FromHexError),
}

/// Domain-separation tag — embedded in every audit-root preimage.
/// Bumping this string is the explicit way to invalidate older
/// anchors (e.g. if a future Phase 3.2 changes the digest shape).
pub const DIGEST_TAG: &str = "sbo3l.audit_root.v1";

/// Compute the audit root over the chain head. `chain_head_event_hash`
/// is the SBO3L `event_hash` field (64 hex chars, no `0x`).
///
/// Returns `0x` + 64 hex chars (32-byte keccak256 digest).
pub fn audit_root(
    network: &str,
    chain_head_seq: u64,
    chain_head_event_hash: &str,
) -> Result<String, AuditRootError> {
    if chain_head_event_hash.is_empty() {
        return Err(AuditRootError::EmptyChain);
    }
    if chain_head_event_hash.len() != 64
        || !chain_head_event_hash.chars().all(|c| c.is_ascii_hexdigit())
    {
        return Err(AuditRootError::BadEventHashShape(
            chain_head_event_hash.to_string(),
        ));
    }
    let mut tip_bytes = [0u8; 32];
    hex::decode_to_slice(chain_head_event_hash, &mut tip_bytes)?;

    let mut hasher = Keccak::v256();
    hasher.update(DIGEST_TAG.as_bytes());
    hasher.update(network.as_bytes());
    hasher.update(&chain_head_seq.to_be_bytes());
    hasher.update(&tip_bytes);
    let mut out = [0u8; 32];
    hasher.finalize(&mut out);
    Ok(format!("0x{}", hex::encode(out)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_event_hash() -> String {
        // 64 hex chars, no 0x prefix — same shape as
        // `audit_events.event_hash` rows in SQLite.
        "a".repeat(64)
    }

    #[test]
    fn audit_root_is_deterministic() {
        let a = audit_root("mainnet", 42, &fixture_event_hash()).unwrap();
        let b = audit_root("mainnet", 42, &fixture_event_hash()).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn audit_root_changes_with_network() {
        let mainnet = audit_root("mainnet", 42, &fixture_event_hash()).unwrap();
        let sepolia = audit_root("sepolia", 42, &fixture_event_hash()).unwrap();
        assert_ne!(
            mainnet, sepolia,
            "audit root MUST differ across networks — anchored chains aren't fungible"
        );
    }

    #[test]
    fn audit_root_changes_with_seq() {
        let head_42 = audit_root("mainnet", 42, &fixture_event_hash()).unwrap();
        let head_43 = audit_root("mainnet", 43, &fixture_event_hash()).unwrap();
        assert_ne!(head_42, head_43);
    }

    #[test]
    fn audit_root_changes_with_tip_hash() {
        let tip_a = audit_root("mainnet", 42, &"a".repeat(64)).unwrap();
        let tip_b = audit_root("mainnet", 42, &"b".repeat(64)).unwrap();
        assert_ne!(tip_a, tip_b);
    }

    #[test]
    fn audit_root_output_is_0x_prefixed_64_hex() {
        let r = audit_root("mainnet", 1, &fixture_event_hash()).unwrap();
        assert!(r.starts_with("0x"));
        assert_eq!(r.len(), 66); // "0x" + 64 hex
        assert!(r[2..].chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn empty_chain_head_event_hash_errors() {
        let err = audit_root("mainnet", 0, "").unwrap_err();
        assert!(matches!(err, AuditRootError::EmptyChain));
    }

    #[test]
    fn bad_shape_event_hash_errors() {
        // 63 chars instead of 64.
        let err = audit_root("mainnet", 1, &"a".repeat(63)).unwrap_err();
        match err {
            AuditRootError::BadEventHashShape(_) => {}
            other => panic!("expected BadEventHashShape, got {other:?}"),
        }
        // 64 chars but contains a non-hex char.
        let mut bad = "a".repeat(63);
        bad.push('z');
        let err = audit_root("mainnet", 1, &bad).unwrap_err();
        assert!(matches!(err, AuditRootError::BadEventHashShape(_)));
    }

    /// Domain-separation tag pin. Bumping this constant is the
    /// explicit way to invalidate older anchors; this test fails
    /// loudly if someone edits the constant by accident.
    #[test]
    fn digest_tag_is_versioned() {
        assert_eq!(DIGEST_TAG, "sbo3l.audit_root.v1");
    }
}
