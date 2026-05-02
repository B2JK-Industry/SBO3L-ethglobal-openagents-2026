//! `AnchorRegistry` contract calldata + dry-run envelope.
//!
//! Wraps Dev 4's [`AnchorRegistry.sol`] contract (PR #245):
//!
//! ```solidity
//! function publishAnchor(
//!     bytes32 tenantId,
//!     bytes32 auditRoot,
//!     uint64 chainHeadBlock
//! ) external returns (uint256 sequence);
//! ```
//!
//! Per-tenant state — each tenant claims a `tenantId` once via
//! `claimTenant`, then publishes anchors at monotonic `sequence`
//! positions. The contract emits an `AnchorPublished` event with
//! `(tenantId, sequence, auditRoot, chainHeadBlock,
//! publishedAt)`.
//!
//! ABI selector = first 4 bytes of
//! `keccak256("publishAnchor(bytes32,bytes32,uint64)")` — pinned
//! at [`PUBLISH_ANCHOR_SELECTOR`] and re-derived in unit tests so
//! a drift can't slip through.
//!
//! Calldata layout (4 + 32 + 32 + 32 = 100 bytes):
//!
//! ```text
//! [00..04)  selector       = 0xa212dc0a
//! [04..36)  tenantId        = 32-byte big-endian
//! [36..68)  auditRoot       = 32-byte big-endian
//! [68..100) chainHeadBlock  = 32-byte big-endian (left-padded uint64)
//! ```
//!
//! # History note (round-10 broadcast pipeline)
//!
//! An earlier scaffold (Phase 3.1 dry-run, PR #246) speculated a
//! 2-arg `writeAnchor(bytes32,uint64)` shape with selector
//! `0xe3bdb043`. Dev 4's actual contract uses the 3-arg
//! `publishAnchor` shape; this module now matches the deployed
//! contract. The broadcast pipeline depends on byte-for-byte
//! agreement between the calldata builder and the deployed
//! contract, so the selector + signature are pinned together with
//! a re-derive test that fails loudly on any drift.

use serde::{Deserialize, Serialize};
use thiserror::Error;
#[cfg(test)]
use tiny_keccak::{Hasher as _, Keccak};

use crate::digest::{audit_root, AuditRootError};

/// Function selector for `publishAnchor(bytes32,bytes32,uint64)`.
/// First 4 bytes of
/// `keccak256("publishAnchor(bytes32,bytes32,uint64)")`. Pinned as
/// a constant + re-derived in
/// [`tests::publish_anchor_selector_matches_signature`].
pub const PUBLISH_ANCHOR_SELECTOR: [u8; 4] = [0xa2, 0x12, 0xdc, 0x0a];

/// Function selector for `claimTenant(bytes32)`. Required as the
/// first call before any `publishAnchor` for a fresh tenant id —
/// the contract revert-guards `publishAnchor` on a non-zero
/// `tenantOwner`. Phase 3.1 broadcast pipeline emits this for the
/// initial-deployment path.
pub const CLAIM_TENANT_SELECTOR: [u8; 4] = [0x79, 0x83, 0xf0, 0xd4];

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
    #[error("anchor: tenant id must be 0x + 64 hex (32 bytes); got `{0}`")]
    BadTenantId(String),
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

fn decode_bytes32_hex(hex_str: &str) -> [u8; 32] {
    let stripped = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    let mut out = [0u8; 32];
    if stripped.len() == 64 {
        let _ = hex::decode_to_slice(stripped, &mut out);
    }
    out
}

/// Build the ABI calldata for
/// `AnchorRegistry.publishAnchor(bytes32 tenantId, bytes32
/// auditRoot, uint64 chainHeadBlock)`. Returns the 100-byte byte
/// sequence ready to wrap in a tx.
///
/// `tenant_id_hex` and `audit_root_hex` are 32-byte values
/// hex-encoded with optional `0x` prefix. `chain_head_block` is
/// the EVM block number the audit-chain digest was computed
/// against — surfaces in the `AnchorPublished` event so an indexer
/// can correlate anchors with on-chain time.
pub fn publish_anchor_calldata(
    tenant_id_hex: &str,
    audit_root_hex: &str,
    chain_head_block: u64,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + 32 + 32 + 32);
    out.extend_from_slice(&PUBLISH_ANCHOR_SELECTOR);
    out.extend_from_slice(&decode_bytes32_hex(tenant_id_hex));
    out.extend_from_slice(&decode_bytes32_hex(audit_root_hex));
    // uint64 left-padded to 32 bytes.
    let mut block_word = [0u8; 32];
    block_word[24..].copy_from_slice(&chain_head_block.to_be_bytes());
    out.extend_from_slice(&block_word);
    out
}

/// Build calldata for `AnchorRegistry.claimTenant(bytes32
/// tenantId)`. Required as the first call before any
/// `publishAnchor` for a fresh tenant id; contract reverts on
/// `publishAnchor` with `tenantOwner == 0`.
pub fn claim_tenant_calldata(tenant_id_hex: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + 32);
    out.extend_from_slice(&CLAIM_TENANT_SELECTOR);
    out.extend_from_slice(&decode_bytes32_hex(tenant_id_hex));
    out
}

/// Off-chain dry-run envelope — the shape `sbo3l audit anchor
/// --dry-run` emits. `broadcasted: false` is the loud honesty
/// marker; `--broadcast` flips it to `true` and adds the `tx_hash`
/// + `block_number` fields the bottom-of-this-file CLI populates.
///
/// **Schema bumped to v2 in round 10**: adds the `tenant_id`
/// field required by Dev 4's contract (`AnchorRegistry.publishAnchor`
/// is per-tenant). v1 envelopes from #246 don't carry tenant_id and
/// can't round-trip into a `publishAnchor` calldata, so the wire
/// shape is intentionally not back-compat.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuditAnchorEnvelope {
    pub schema: String,
    pub network: String,
    /// EIP-55 mixed-case hex with `0x` prefix.
    pub registry_address: String,
    /// `0x` + 64 hex — the tenant id the anchor is published for.
    /// Operators with a single tenant typically use
    /// `keccak256("default")` or a stable opaque id; multi-tenant
    /// deployments map their `TenantId` (per V010) to a 32-byte
    /// commitment.
    pub tenant_id: String,
    /// `0x` + 64 hex of the keccak256 digest from
    /// [`crate::digest::audit_root`].
    pub audit_root: String,
    /// Internal SBO3L sequence — the audit chain seq at which this
    /// digest was computed. Distinct from `chain_head_block` (EVM
    /// block number) in `publishAnchor`'s contract args.
    pub chain_head_seq: u64,
    /// EVM block number the digest is being anchored against.
    /// Surfaces in `AnchorPublished` event for indexer correlation.
    pub chain_head_block: u64,
    /// Raw 64-char hex (no `0x`) of the SBO3L `event_hash` at the
    /// chain tip. Mirrors `audit_events.event_hash`.
    pub chain_head_event_hash: String,
    /// RFC3339 timestamp when the envelope was computed.
    pub computed_at: String,
    /// `0x` + 200 hex (4-byte selector + 3 × 32-byte words) ABI
    /// calldata for `publishAnchor(bytes32, bytes32, uint64)`.
    pub publish_anchor_calldata_hex: String,
    /// Honest disclosure — dry-runs do NOT contact an RPC.
    pub broadcasted: bool,
}

pub const AUDIT_ANCHOR_ENVELOPE_SCHEMA: &str = "sbo3l.audit_anchor_envelope.v2";

/// Build a full dry-run envelope from a chain-head observation.
/// `chain_head_event_hash` is the raw 64-char hex string from the
/// `audit_events.event_hash` column. `chain_head_block` is the EVM
/// block number the digest is being anchored against (defaults to
/// `0` if the daemon doesn't track block numbers — operators
/// running the publish job from CI typically pass `eth_blockNumber`
/// at job-start time).
#[allow(clippy::too_many_arguments)]
pub fn build_dry_run_envelope(
    network: AuditAnchorNetwork,
    tenant_id_hex: &str,
    chain_head_seq: u64,
    chain_head_block: u64,
    chain_head_event_hash: &str,
    registry_address_override: Option<&str>,
    computed_at_rfc3339: &str,
) -> Result<AuditAnchorEnvelope, AnchorRegistryError> {
    let registry = registry_address_override.unwrap_or(network.default_registry());
    validate_registry(registry)?;
    validate_tenant_id(tenant_id_hex)?;

    let root = audit_root(network.as_str(), chain_head_seq, chain_head_event_hash)?;
    let calldata = publish_anchor_calldata(tenant_id_hex, &root, chain_head_block);

    Ok(AuditAnchorEnvelope {
        schema: AUDIT_ANCHOR_ENVELOPE_SCHEMA.to_string(),
        network: network.as_str().to_string(),
        registry_address: registry.to_string(),
        tenant_id: tenant_id_hex.to_string(),
        audit_root: root,
        chain_head_seq,
        chain_head_block,
        chain_head_event_hash: chain_head_event_hash.to_string(),
        computed_at: computed_at_rfc3339.to_string(),
        publish_anchor_calldata_hex: format!("0x{}", hex::encode(&calldata)),
        broadcasted: false,
    })
}

fn validate_tenant_id(tenant_id_hex: &str) -> Result<(), AnchorRegistryError> {
    let stripped = tenant_id_hex.strip_prefix("0x").unwrap_or(tenant_id_hex);
    if stripped.len() != 64 || !stripped.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(AnchorRegistryError::BadTenantId(tenant_id_hex.to_string()));
    }
    Ok(())
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

    fn fixture_tenant() -> String {
        format!("0x{}", "b".repeat(64))
    }

    /// Selector pin for `publishAnchor` — recompute from the
    /// canonical signature. Catches both selector drift AND a
    /// signature-string typo.
    #[test]
    fn publish_anchor_selector_matches_signature() {
        let derived = keccak256(b"publishAnchor(bytes32,bytes32,uint64)");
        assert_eq!(
            derived[..4],
            PUBLISH_ANCHOR_SELECTOR,
            "PUBLISH_ANCHOR_SELECTOR drifted from keccak256 of Dev 4's canonical signature"
        );
    }

    #[test]
    fn claim_tenant_selector_matches_signature() {
        let derived = keccak256(b"claimTenant(bytes32)");
        assert_eq!(derived[..4], CLAIM_TENANT_SELECTOR);
    }

    #[test]
    fn publish_calldata_layout_is_100_bytes_with_three_words() {
        let root = audit_root("mainnet", 42, &fixture_tip()).unwrap();
        let cd = publish_anchor_calldata(&fixture_tenant(), &root, 1234);
        assert_eq!(cd.len(), 4 + 32 + 32 + 32);
        assert_eq!(cd[..4], PUBLISH_ANCHOR_SELECTOR);
        // chain_head_block lives at bytes 68..100; low 8 bytes
        // carry the BE u64.
        assert_eq!(&cd[92..100], &1234_u64.to_be_bytes());
        // bytes 68..92 must be zero (uint64 left-padded).
        assert_eq!(&cd[68..92], &[0u8; 24]);
    }

    #[test]
    fn publish_calldata_audit_root_round_trips() {
        let root = audit_root("mainnet", 42, &fixture_tip()).unwrap();
        let cd = publish_anchor_calldata(&fixture_tenant(), &root, 0);
        // Bytes 36..68 = raw 32-byte root.
        let mut expected = [0u8; 32];
        hex::decode_to_slice(&root[2..], &mut expected).unwrap();
        assert_eq!(&cd[36..68], &expected);
    }

    #[test]
    fn publish_calldata_tenant_id_round_trips() {
        let tenant = fixture_tenant();
        let root = audit_root("mainnet", 1, &fixture_tip()).unwrap();
        let cd = publish_anchor_calldata(&tenant, &root, 0);
        // Bytes 4..36 = raw 32-byte tenant id.
        let mut expected = [0u8; 32];
        hex::decode_to_slice(&tenant[2..], &mut expected).unwrap();
        assert_eq!(&cd[4..36], &expected);
    }

    #[test]
    fn claim_tenant_calldata_layout_is_36_bytes() {
        let tenant = fixture_tenant();
        let cd = claim_tenant_calldata(&tenant);
        assert_eq!(cd.len(), 4 + 32);
        assert_eq!(cd[..4], CLAIM_TENANT_SELECTOR);
        let mut expected = [0u8; 32];
        hex::decode_to_slice(&tenant[2..], &mut expected).unwrap();
        assert_eq!(&cd[4..36], &expected);
    }

    #[test]
    fn build_dry_run_envelope_produces_stable_shape() {
        let env = build_dry_run_envelope(
            AuditAnchorNetwork::Mainnet,
            &fixture_tenant(),
            42,
            1234,
            &fixture_tip(),
            None,
            "2026-05-02T10:00:00Z",
        )
        .unwrap();
        assert_eq!(env.schema, AUDIT_ANCHOR_ENVELOPE_SCHEMA);
        assert_eq!(env.schema, "sbo3l.audit_anchor_envelope.v2");
        assert_eq!(env.network, "mainnet");
        assert_eq!(env.registry_address, ANCHOR_REGISTRY_MAINNET);
        assert_eq!(env.tenant_id, fixture_tenant());
        assert!(env.audit_root.starts_with("0x") && env.audit_root.len() == 66);
        assert_eq!(env.chain_head_seq, 42);
        assert_eq!(env.chain_head_block, 1234);
        assert!(env.publish_anchor_calldata_hex.starts_with("0x"));
        // 4 + 32 + 32 + 32 = 100 bytes = 200 hex + "0x" prefix.
        assert_eq!(env.publish_anchor_calldata_hex.len(), 202);
        assert!(!env.broadcasted, "dry-run never marks broadcasted");
    }

    #[test]
    fn build_dry_run_envelope_propagates_audit_root_errors() {
        let err = build_dry_run_envelope(
            AuditAnchorNetwork::Mainnet,
            &fixture_tenant(),
            42,
            0,
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
            &fixture_tenant(),
            42,
            0,
            &fixture_tip(),
            Some("not-a-hex-address"),
            "2026-05-02T10:00:00Z",
        )
        .unwrap_err();
        assert!(matches!(err, AnchorRegistryError::BadRegistryAddress(_)));
    }

    #[test]
    fn build_dry_run_envelope_rejects_bad_tenant_id() {
        let err = build_dry_run_envelope(
            AuditAnchorNetwork::Mainnet,
            "not-32-bytes",
            42,
            0,
            &fixture_tip(),
            None,
            "2026-05-02T10:00:00Z",
        )
        .unwrap_err();
        assert!(matches!(err, AnchorRegistryError::BadTenantId(_)));
    }

    #[test]
    fn build_dry_run_envelope_accepts_explicit_registry_override() {
        let override_addr = "0x1234567890abcdef1234567890abcdef12345678";
        let env = build_dry_run_envelope(
            AuditAnchorNetwork::Sepolia,
            &fixture_tenant(),
            1,
            0,
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
            &fixture_tenant(),
            42,
            1234,
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
