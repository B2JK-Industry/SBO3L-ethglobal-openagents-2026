//! `AnchorRegistry` contract calldata + dry-run envelope.
//!
//! On-chain shape (see `crates/sbo3l-anchor/contracts/AnchorRegistry.sol`
//! follow-up):
//!
//! ```solidity
//! function writeAnchor(bytes32 auditRoot, uint64 chainHeadSeq) external {
//!     anchors[keccak256(abi.encodePacked(auditRoot, chainHeadSeq))]
//!         = block.timestamp;
//!     emit AnchorWritten(auditRoot, chainHeadSeq, block.timestamp);
//! }
//! ```
//!
//! ABI selector = first 4 bytes of
//! `keccak256("writeAnchor(bytes32,uint64)")` — pinned at
//! [`WRITE_ANCHOR_SELECTOR`] and re-derived in unit tests so a
//! drift can't slip through.
//!
//! Calldata layout (4 + 32 + 32 = 68 bytes):
//!
//! ```text
//! [00..04) selector  = 0x<WRITE_ANCHOR_SELECTOR>
//! [04..36) auditRoot = 32-byte big-endian
//! [36..68) seq       = 32-byte big-endian (left-padded uint64)
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;
#[cfg(test)]
use tiny_keccak::{Hasher as _, Keccak};

use crate::digest::{audit_root, AuditRootError};

/// Function selector for `writeAnchor(bytes32,uint64)`. First 4
/// bytes of `keccak256("writeAnchor(bytes32,uint64)")`. Pinned as a
/// constant + re-derived in [`tests::write_anchor_selector_matches_signature`].
pub const WRITE_ANCHOR_SELECTOR: [u8; 4] = [0xe3, 0xbd, 0xb0, 0x43];

/// Mainnet AnchorRegistry — placeholder address. Replaced with the
/// real deployment once the contract ships.
pub const ANCHOR_REGISTRY_MAINNET: &str = "0x0000000000000000000000000000000000000000";

/// Sepolia AnchorRegistry — placeholder. Same pattern.
pub const ANCHOR_REGISTRY_SEPOLIA: &str = "0x0000000000000000000000000000000000000000";

// PartialEq/Eq omitted — wraps AuditRootError which can't impl Eq
// (hex::FromHexError doesn't). Callers should match on the variant
// shape, not compare on equality.
#[derive(Debug, Error)]
pub enum AnchorRegistryError {
    #[error("anchor: {0}")]
    Digest(#[from] AuditRootError),
    #[error("anchor: registry address must be 0x + 40 hex; got `{0}`")]
    BadRegistryAddress(String),
    #[error("anchor: unsupported network `{0}` (expected `mainnet` | `sepolia`)")]
    UnsupportedNetwork(String),
}

/// Network discriminator. Mirrors `sbo3l_identity::EnsNetwork` but
/// owned by this crate so anchor consumers don't pull the identity
/// crate.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AuditAnchorNetwork {
    Mainnet,
    Sepolia,
}

impl AuditAnchorNetwork {
    pub fn parse(s: &str) -> Result<Self, AnchorRegistryError> {
        match s {
            "mainnet" => Ok(Self::Mainnet),
            "sepolia" => Ok(Self::Sepolia),
            other => Err(AnchorRegistryError::UnsupportedNetwork(other.to_string())),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mainnet => "mainnet",
            Self::Sepolia => "sepolia",
        }
    }
    pub fn default_registry(self) -> &'static str {
        match self {
            Self::Mainnet => ANCHOR_REGISTRY_MAINNET,
            Self::Sepolia => ANCHOR_REGISTRY_SEPOLIA,
        }
    }
}

/// Build the ABI calldata for
/// `AnchorRegistry.writeAnchor(bytes32, uint64)`. Returns the
/// 68-byte byte sequence ready to wrap in a tx.
pub fn write_anchor_calldata(audit_root_hex: &str, chain_head_seq: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + 32 + 32);
    out.extend_from_slice(&WRITE_ANCHOR_SELECTOR);

    // arg 0: bytes32 auditRoot. The digest is already 32 bytes;
    // strip "0x" prefix if present.
    let stripped = audit_root_hex.strip_prefix("0x").unwrap_or(audit_root_hex);
    let mut root_bytes = [0u8; 32];
    if stripped.len() == 64 {
        // Best-effort decode; caller-supplied invariants checked
        // upstream in `build_dry_run_envelope`.
        let _ = hex::decode_to_slice(stripped, &mut root_bytes);
    }
    out.extend_from_slice(&root_bytes);

    // arg 1: uint64 chainHeadSeq, left-padded to 32 bytes.
    let mut seq_word = [0u8; 32];
    seq_word[24..].copy_from_slice(&chain_head_seq.to_be_bytes());
    out.extend_from_slice(&seq_word);

    out
}

/// Off-chain dry-run envelope — the shape `sbo3l audit anchor
/// --dry-run` would emit. `broadcasted: false` is the loud honesty
/// marker; the broadcast follow-up flips it to `true` and adds a
/// `tx_hash` field.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuditAnchorEnvelope {
    pub schema: String,
    pub network: String,
    /// EIP-55 mixed-case hex with `0x` prefix.
    pub registry_address: String,
    /// `0x` + 64 hex of the keccak256 digest from
    /// [`crate::digest::audit_root`].
    pub audit_root: String,
    pub chain_head_seq: u64,
    /// Raw 64-char hex (no `0x`) of the SBO3L `event_hash` at the
    /// chain tip. Mirrors what `audit_events.event_hash` stores.
    pub chain_head_event_hash: String,
    /// RFC3339 timestamp when the envelope was computed.
    pub computed_at: String,
    /// `0x` + 136 hex (4-byte selector + 32 + 32) ABI calldata for
    /// `writeAnchor(bytes32, uint64)`.
    pub write_anchor_calldata_hex: String,
    /// Honest disclosure — dry-runs do NOT contact an RPC.
    pub broadcasted: bool,
}

pub const AUDIT_ANCHOR_ENVELOPE_SCHEMA: &str = "sbo3l.audit_anchor_envelope.v1";

/// Build a full dry-run envelope from a chain-head observation.
/// `chain_head_event_hash` is the raw 64-char hex string from the
/// `audit_events.event_hash` column.
pub fn build_dry_run_envelope(
    network: AuditAnchorNetwork,
    chain_head_seq: u64,
    chain_head_event_hash: &str,
    registry_address_override: Option<&str>,
    computed_at_rfc3339: &str,
) -> Result<AuditAnchorEnvelope, AnchorRegistryError> {
    let registry = registry_address_override.unwrap_or(network.default_registry());
    validate_registry(registry)?;

    let root = audit_root(network.as_str(), chain_head_seq, chain_head_event_hash)?;
    let calldata = write_anchor_calldata(&root, chain_head_seq);

    Ok(AuditAnchorEnvelope {
        schema: AUDIT_ANCHOR_ENVELOPE_SCHEMA.to_string(),
        network: network.as_str().to_string(),
        registry_address: registry.to_string(),
        audit_root: root,
        chain_head_seq,
        chain_head_event_hash: chain_head_event_hash.to_string(),
        computed_at: computed_at_rfc3339.to_string(),
        write_anchor_calldata_hex: format!("0x{}", hex::encode(&calldata)),
        broadcasted: false,
    })
}

fn validate_registry(addr: &str) -> Result<(), AnchorRegistryError> {
    let stripped = addr.strip_prefix("0x").unwrap_or(addr);
    if stripped.len() != 40 || !stripped.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(AnchorRegistryError::BadRegistryAddress(addr.to_string()));
    }
    Ok(())
}

/// Local keccak256 helper. Used by the selector pin test in this
/// module; gated to `cfg(test)` so non-test builds don't carry it.
#[cfg(test)]
fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut h = Keccak::v256();
    h.update(data);
    let mut out = [0u8; 32];
    h.finalize(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_tip() -> String {
        "a".repeat(64)
    }

    /// Selector pin — recompute from the canonical signature. If the
    /// constant drifts (or the signature changes), this fails.
    #[test]
    fn write_anchor_selector_matches_signature() {
        let derived = keccak256(b"writeAnchor(bytes32,uint64)");
        assert_eq!(
            derived[..4],
            WRITE_ANCHOR_SELECTOR,
            "WRITE_ANCHOR_SELECTOR drifted from keccak256 of canonical signature"
        );
    }

    #[test]
    fn calldata_layout_is_4_plus_32_plus_32_bytes() {
        let root = audit_root("mainnet", 42, &fixture_tip()).unwrap();
        let cd = write_anchor_calldata(&root, 42);
        assert_eq!(cd.len(), 4 + 32 + 32);
        assert_eq!(cd[..4], WRITE_ANCHOR_SELECTOR);
        // arg 1 (seq) lives at bytes 36..68; the 8 low bytes carry
        // the big-endian u64.
        assert_eq!(&cd[60..68], &42_u64.to_be_bytes());
        // bytes 36..60 of the seq word must be zero (uint64
        // left-padded into 32 bytes).
        assert_eq!(&cd[36..60], &[0u8; 24]);
    }

    #[test]
    fn calldata_audit_root_round_trips() {
        let root = audit_root("mainnet", 42, &fixture_tip()).unwrap();
        let cd = write_anchor_calldata(&root, 42);
        // Bytes 4..36 should equal the raw 32-byte root hex-decoded.
        let mut expected = [0u8; 32];
        hex::decode_to_slice(&root[2..], &mut expected).unwrap();
        assert_eq!(&cd[4..36], &expected);
    }

    #[test]
    fn build_dry_run_envelope_produces_stable_shape() {
        let env = build_dry_run_envelope(
            AuditAnchorNetwork::Mainnet,
            42,
            &fixture_tip(),
            None,
            "2026-05-02T10:00:00Z",
        )
        .unwrap();
        assert_eq!(env.schema, AUDIT_ANCHOR_ENVELOPE_SCHEMA);
        assert_eq!(env.network, "mainnet");
        assert_eq!(env.registry_address, ANCHOR_REGISTRY_MAINNET);
        assert!(env.audit_root.starts_with("0x") && env.audit_root.len() == 66);
        assert_eq!(env.chain_head_seq, 42);
        assert!(env.write_anchor_calldata_hex.starts_with("0x"));
        // 4 + 32 + 32 = 68 bytes = 136 hex + "0x" prefix
        assert_eq!(env.write_anchor_calldata_hex.len(), 138);
        assert!(!env.broadcasted, "dry-run never marks broadcasted");
    }

    #[test]
    fn build_dry_run_envelope_propagates_audit_root_errors() {
        let err = build_dry_run_envelope(
            AuditAnchorNetwork::Mainnet,
            42,
            "", // empty tip
            None,
            "2026-05-02T10:00:00Z",
        )
        .unwrap_err();
        assert!(matches!(err, AnchorRegistryError::Digest(_)));
    }

    #[test]
    fn build_dry_run_envelope_rejects_bad_registry_override() {
        let err = build_dry_run_envelope(
            AuditAnchorNetwork::Mainnet,
            42,
            &fixture_tip(),
            Some("not-a-hex-address"),
            "2026-05-02T10:00:00Z",
        )
        .unwrap_err();
        assert!(matches!(err, AnchorRegistryError::BadRegistryAddress(_)));
    }

    #[test]
    fn build_dry_run_envelope_accepts_explicit_registry_override() {
        let override_addr = "0x1234567890abcdef1234567890abcdef12345678";
        let env = build_dry_run_envelope(
            AuditAnchorNetwork::Sepolia,
            1,
            &fixture_tip(),
            Some(override_addr),
            "2026-05-02T10:00:00Z",
        )
        .unwrap();
        assert_eq!(env.registry_address, override_addr);
    }

    #[test]
    fn network_parse_round_trip() {
        assert_eq!(
            AuditAnchorNetwork::parse("mainnet").unwrap(),
            AuditAnchorNetwork::Mainnet
        );
        assert_eq!(
            AuditAnchorNetwork::parse("sepolia").unwrap(),
            AuditAnchorNetwork::Sepolia
        );
        let err = AuditAnchorNetwork::parse("polygon").unwrap_err();
        assert!(matches!(err, AnchorRegistryError::UnsupportedNetwork(_)));
    }

    #[test]
    fn envelope_roundtrips_via_serde() {
        let env = build_dry_run_envelope(
            AuditAnchorNetwork::Mainnet,
            42,
            &fixture_tip(),
            None,
            "2026-05-02T10:00:00Z",
        )
        .unwrap();
        let s = serde_json::to_string(&env).unwrap();
        let back: AuditAnchorEnvelope = serde_json::from_str(&s).unwrap();
        assert_eq!(back, env);
    }
}
