//! ENS NameWrapper integration helpers (R13 P6).
//!
//! NameWrapper (`0xD4416b13d2b3a9aBae7AcD5D6C2BbDBE25686401` mainnet,
//! `0x0635513f179D50A207757E05759CbD106d7dFcE8` Sepolia) wraps a
//! `.eth` name so its `setSubnodeRecord` / `setText` calls become
//! ERC-1155 transfers, and the parent owner can **burn fuses** to
//! permanently waive specific powers (e.g. CANNOT_UNWRAP, CANNOT_TRANSFER,
//! CANNOT_SET_RESOLVER, PARENT_CANNOT_CONTROL).
//!
//! ## Why SBO3L wraps
//!
//! Wrapping `sbo3lagent.eth` in NameWrapper buys two properties:
//!
//! 1. **Trustless subname delegation.** Setting `PARENT_CANNOT_CONTROL`
//!    on a subname (after issuing it) makes the subname's owner
//!    permanently sovereign — the parent can no longer recall the
//!    subname or rotate its records. This is the key property an
//!    agent operator wants when receiving an SBO3L subname: "the
//!    SBO3L team can't pull my agent identity from under me."
//! 2. **Burnable parent fuses.** Burning `CANNOT_UNWRAP` on
//!    `sbo3lagent.eth` itself locks the parent into a wrapped
//!    state — Daniel can no longer accidentally unwrap and lose
//!    the trustless-subname guarantees the children depend on.
//!
//! ## Scope of this module
//!
//! Pure-function calldata builders + per-network address pins.
//! No live wrap operations from Rust — those run via `cast send` or
//! a forge script; the operator wraps once. The Rust module:
//!
//! - Exposes `NameWrapper` address constants per network.
//! - Defines `Fuse` flag set with the canonical bit values.
//! - Builds `wrapETH2LD` / `setSubnodeRecord` / `setFuses` calldata
//!   so SBO3L tooling can emit dry-run envelopes for the operator
//!   to inspect before broadcasting.
//!
//! ## Trust model
//!
//! Same as the rest of the SBO3L ENS surface: NameWrapper is
//! public infrastructure; we read + emit calldata, never custody.
//! Operator-side multisig is the right deploy posture for the
//! parent wrap operation.

use thiserror::Error;
#[cfg(test)]
use tiny_keccak::{Hasher, Keccak};

use crate::ens_anchor::{namehash, AnchorError};

/// Mainnet NameWrapper address.
pub const NAME_WRAPPER_MAINNET: &str = "0xD4416b13d2b3a9aBae7AcD5D6C2BbDBE25686401";

/// Sepolia NameWrapper address.
pub const NAME_WRAPPER_SEPOLIA: &str = "0x0635513f179D50A207757E05759CbD106d7dFcE8";

/// `wrapETH2LD(string label, address wrappedOwner, uint16 ownerControlledFuses, address resolver)`
/// — wraps a top-level `.eth` second-level domain.
/// Pinned to `keccak256("wrapETH2LD(string,address,uint16,address)")[..4]`.
pub const WRAP_ETH_2LD_SELECTOR: [u8; 4] = [0x8c, 0xf8, 0xb4, 0x1e];

/// `setSubnodeRecord(bytes32 parentNode, string label, address newOwner, address resolver, uint64 ttl, uint32 fuses, uint64 expiry)`
/// — issue a subname under a wrapped parent, optionally burning child fuses
/// in the same call. Pinned to
/// `keccak256("setSubnodeRecord(bytes32,string,address,address,uint64,uint32,uint64)")[..4]`.
pub const NW_SET_SUBNODE_RECORD_SELECTOR: [u8; 4] = [0x24, 0xc1, 0xaf, 0x44];

/// `setFuses(bytes32 node, uint16 ownerControlledFuses)` — burn fuses
/// on an already-wrapped name. Pinned to
/// `keccak256("setFuses(bytes32,uint16)")[..4]`.
pub const SET_FUSES_SELECTOR: [u8; 4] = [0x40, 0x29, 0x06, 0xfc];

/// Canonical NameWrapper fuses. Pinned by recompute against the ENS
/// docs (`docs.ens.domains/wrapper/fuses`). The bit positions are
/// stable contract-of-trust; bumping them would break every wrapped
/// name on chain, so they're load-bearing constants.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fuse {
    /// Owner can no longer unwrap. Locks the name in NameWrapper
    /// permanently. Required before children can rely on
    /// `PARENT_CANNOT_CONTROL`.
    CANNOT_UNWRAP = 0x0001,
    /// Owner can no longer burn additional fuses. Useful as a
    /// "freeze the policy" final step.
    CANNOT_BURN_FUSES = 0x0002,
    /// Owner can no longer transfer the wrapped name (ERC-1155
    /// transfer). Pinned-owner property.
    CANNOT_TRANSFER = 0x0004,
    /// Owner can no longer change the resolver address.
    CANNOT_SET_RESOLVER = 0x0008,
    /// Owner can no longer change the TTL.
    CANNOT_SET_TTL = 0x0010,
    /// Owner can no longer create new subnames.
    CANNOT_CREATE_SUBDOMAIN = 0x0020,
    /// Owner can no longer approve other addresses to manage records.
    CANNOT_APPROVE = 0x0040,
    /// **Parent can no longer control this name.** Set on a CHILD
    /// to make the child sovereign — parent can't recall, rename,
    /// or re-issue. Requires `CANNOT_UNWRAP` on the same child.
    PARENT_CANNOT_CONTROL = 0x10000,
    /// Reserved fuses 0x20000+ are NameWrapper-internal.
    IS_DOT_ETH = 0x20000,
    /// Reserved.
    CAN_EXTEND_EXPIRY = 0x40000,
}

impl Fuse {
    pub const fn bit(self) -> u32 {
        self as u32
    }
}

/// Combine a list of fuses into the bitmask the contract expects.
///
/// Operator-controlled fuses (the ones in the `ownerControlledFuses`
/// uint16 arg of `wrapETH2LD` and `setFuses`) live in the low 16
/// bits. Parent-controlled fuses (`PARENT_CANNOT_CONTROL`,
/// `IS_DOT_ETH`, `CAN_EXTEND_EXPIRY`) live in the high bits and
/// pass through `setSubnodeRecord`'s `fuses` uint32 arg instead.
pub fn fuses_bitmask(fuses: &[Fuse]) -> u32 {
    let mut out = 0u32;
    for f in fuses {
        out |= f.bit();
    }
    out
}

/// NameWrapper-side errors. Mirrors `AnchorError` shape so callers
/// can flatten error types easily.
#[derive(Debug, Error)]
pub enum NameWrapperError {
    #[error("label must be a single non-empty DNS label (no `.`)")]
    InvalidLabel,
    #[error("label too long: {0} bytes (max 255)")]
    LabelTooLong(usize),
    #[error("operator-controlled fuses must fit in uint16")]
    OwnerFusesOverflow,
    #[error(transparent)]
    Anchor(#[from] AnchorError),
}

#[cfg(test)]
fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut h = Keccak::v256();
    h.update(data);
    let mut out = [0u8; 32];
    h.finalize(&mut out);
    out
}

fn u256_be(n: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[24..].copy_from_slice(&n.to_be_bytes());
    out
}

fn u256_be_u32(n: u32) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[28..].copy_from_slice(&n.to_be_bytes());
    out
}

fn pad_to_32_multiple(n: usize) -> usize {
    n.div_ceil(32) * 32
}

fn validate_label(label: &str) -> Result<(), NameWrapperError> {
    if label.is_empty() || label.contains('.') {
        return Err(NameWrapperError::InvalidLabel);
    }
    if label.len() > 255 {
        return Err(NameWrapperError::LabelTooLong(label.len()));
    }
    Ok(())
}

fn parse_address(hex: &str) -> Result<[u8; 20], NameWrapperError> {
    let stripped = hex
        .strip_prefix("0x")
        .or_else(|| hex.strip_prefix("0X"))
        .unwrap_or(hex);
    if stripped.len() != 40 {
        return Err(NameWrapperError::Anchor(AnchorError::ResolverBadFormat(
            hex.to_string(),
        )));
    }
    let bytes = ::hex::decode(stripped)
        .map_err(|_| NameWrapperError::Anchor(AnchorError::ResolverBadFormat(hex.to_string())))?;
    let mut out = [0u8; 20];
    out.copy_from_slice(&bytes);
    Ok(out)
}

/// ABI-encode `wrapETH2LD(string label, address wrappedOwner, uint16 ownerControlledFuses, address resolver)`.
///
/// `owner_fuses` MUST fit in uint16 — only the low-16-bit
/// operator-controlled fuses are settable here. Parent-controlled
/// high-bit fuses (`PARENT_CANNOT_CONTROL`, `IS_DOT_ETH`) cannot
/// be burned via `wrapETH2LD`; use `setSubnodeRecord` for those.
pub fn wrap_eth_2ld_calldata(
    label: &str,
    wrapped_owner: &str,
    owner_fuses: u32,
    resolver: &str,
) -> Result<Vec<u8>, NameWrapperError> {
    validate_label(label)?;
    if owner_fuses > u16::MAX as u32 {
        return Err(NameWrapperError::OwnerFusesOverflow);
    }
    let owner_addr = parse_address(wrapped_owner)?;
    let resolver_addr = parse_address(resolver)?;

    let label_bytes = label.as_bytes();
    let label_padded = pad_to_32_multiple(label_bytes.len());

    let mut out = Vec::with_capacity(4 + 32 * 4 + 32 + label_padded);
    out.extend_from_slice(&WRAP_ETH_2LD_SELECTOR);

    // arg 0 head: string offset = 0x80 (4 head words after selector).
    out.extend_from_slice(&u256_be(0x80));
    // arg 1: address (left-pad to 32 bytes).
    let mut owner_word = [0u8; 32];
    owner_word[12..32].copy_from_slice(&owner_addr);
    out.extend_from_slice(&owner_word);
    // arg 2: uint16 fuses (right-padded to 32 bytes).
    out.extend_from_slice(&u256_be_u32(owner_fuses));
    // arg 3: address.
    let mut resolver_word = [0u8; 32];
    resolver_word[12..32].copy_from_slice(&resolver_addr);
    out.extend_from_slice(&resolver_word);

    // Tail for arg 0: length || padded bytes.
    out.extend_from_slice(&u256_be(label_bytes.len() as u64));
    out.extend_from_slice(label_bytes);
    let pad = label_padded - label_bytes.len();
    out.extend(std::iter::repeat_n(0u8, pad));

    Ok(out)
}

/// ABI-encode `setFuses(bytes32 node, uint16 ownerControlledFuses)`.
pub fn set_fuses_calldata(domain: &str, owner_fuses: u32) -> Result<Vec<u8>, NameWrapperError> {
    if owner_fuses > u16::MAX as u32 {
        return Err(NameWrapperError::OwnerFusesOverflow);
    }
    let node = namehash(domain).map_err(NameWrapperError::Anchor)?;
    let mut out = Vec::with_capacity(4 + 64);
    out.extend_from_slice(&SET_FUSES_SELECTOR);
    out.extend_from_slice(&node);
    out.extend_from_slice(&u256_be_u32(owner_fuses));
    Ok(out)
}

/// ABI-encode NameWrapper's
/// `setSubnodeRecord(bytes32 parentNode, string label, address newOwner, address resolver, uint64 ttl, uint32 fuses, uint64 expiry)`.
///
/// Distinct from the bare ENS Registry's `setSubnodeRecord` — this
/// one carries the `fuses` and `expiry` extra args. Use this when
/// issuing a child under a wrapped parent.
pub fn nw_set_subnode_record_calldata(
    parent_domain: &str,
    label: &str,
    new_owner: &str,
    resolver: &str,
    ttl: u64,
    fuses: u32,
    expiry: u64,
) -> Result<Vec<u8>, NameWrapperError> {
    validate_label(label)?;
    let parent_node = namehash(parent_domain).map_err(NameWrapperError::Anchor)?;
    let owner_addr = parse_address(new_owner)?;
    let resolver_addr = parse_address(resolver)?;

    let label_bytes = label.as_bytes();
    let label_padded = pad_to_32_multiple(label_bytes.len());

    // 7 head words + label tail.
    let mut out = Vec::with_capacity(4 + 32 * 7 + 32 + label_padded);
    out.extend_from_slice(&NW_SET_SUBNODE_RECORD_SELECTOR);

    // arg 0: bytes32 parentNode (inline).
    out.extend_from_slice(&parent_node);
    // arg 1 head: string offset = 7 head words × 32 = 0xe0.
    out.extend_from_slice(&u256_be(0xe0));
    // arg 2: address newOwner.
    let mut owner_word = [0u8; 32];
    owner_word[12..32].copy_from_slice(&owner_addr);
    out.extend_from_slice(&owner_word);
    // arg 3: address resolver.
    let mut resolver_word = [0u8; 32];
    resolver_word[12..32].copy_from_slice(&resolver_addr);
    out.extend_from_slice(&resolver_word);
    // arg 4: uint64 ttl.
    out.extend_from_slice(&u256_be(ttl));
    // arg 5: uint32 fuses (full-32 mask, both operator + parent bits).
    out.extend_from_slice(&u256_be_u32(fuses));
    // arg 6: uint64 expiry.
    out.extend_from_slice(&u256_be(expiry));

    // Tail for arg 1: length || padded bytes.
    out.extend_from_slice(&u256_be(label_bytes.len() as u64));
    out.extend_from_slice(label_bytes);
    let pad = label_padded - label_bytes.len();
    out.extend(std::iter::repeat_n(0u8, pad));

    Ok(out)
}

/// Per-network NameWrapper address. Mirrors `EnsNetwork`'s shape.
pub fn name_wrapper_for(network: &str) -> Option<&'static str> {
    match network {
        "mainnet" => Some(NAME_WRAPPER_MAINNET),
        "sepolia" => Some(NAME_WRAPPER_SEPOLIA),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_eth_2ld_selector_matches_keccak() {
        let h = keccak256(b"wrapETH2LD(string,address,uint16,address)");
        assert_eq!(&h[..4], &WRAP_ETH_2LD_SELECTOR);
    }

    #[test]
    fn set_fuses_selector_matches_keccak() {
        let h = keccak256(b"setFuses(bytes32,uint16)");
        assert_eq!(&h[..4], &SET_FUSES_SELECTOR);
    }

    #[test]
    fn nw_set_subnode_record_selector_matches_keccak() {
        let h = keccak256(b"setSubnodeRecord(bytes32,string,address,address,uint64,uint32,uint64)");
        assert_eq!(&h[..4], &NW_SET_SUBNODE_RECORD_SELECTOR);
    }

    #[test]
    fn fuses_bitmask_combines_correctly() {
        let bits = fuses_bitmask(&[Fuse::CANNOT_UNWRAP, Fuse::CANNOT_TRANSFER]);
        assert_eq!(bits, 0x0001 | 0x0004);
    }

    #[test]
    fn fuses_bitmask_handles_parent_fuses() {
        let bits = fuses_bitmask(&[
            Fuse::CANNOT_UNWRAP,
            Fuse::PARENT_CANNOT_CONTROL,
            Fuse::IS_DOT_ETH,
        ]);
        assert_eq!(bits, 0x0001 | 0x10000 | 0x20000);
    }

    #[test]
    fn wrap_eth_2ld_happy_path() {
        let cd = wrap_eth_2ld_calldata(
            "sbo3lagent",
            "0xdc7EFA6b4Bd77d1a406DE4727F0DF567e597D231",
            fuses_bitmask(&[Fuse::CANNOT_UNWRAP]),
            "0x231b0Ee14048e9dCcD1d247744d114a4EB5E8E63",
        )
        .unwrap();
        assert_eq!(&cd[..4], &WRAP_ETH_2LD_SELECTOR);
        // 4 selector + 4 head words (32 each) + length word + padded label
        // For "sbo3lagent" = 10 bytes, padded to 32.
        assert_eq!(cd.len(), 4 + 32 * 4 + 32 + 32);
    }

    #[test]
    fn wrap_eth_2ld_rejects_dotted_label() {
        let err = wrap_eth_2ld_calldata(
            "foo.bar",
            "0xdc7EFA6b4Bd77d1a406DE4727F0DF567e597D231",
            0,
            "0x231b0Ee14048e9dCcD1d247744d114a4EB5E8E63",
        )
        .unwrap_err();
        assert!(matches!(err, NameWrapperError::InvalidLabel));
    }

    #[test]
    fn wrap_eth_2ld_rejects_owner_fuses_above_u16() {
        let err = wrap_eth_2ld_calldata(
            "foo",
            "0xdc7EFA6b4Bd77d1a406DE4727F0DF567e597D231",
            // Parent fuses (high-bit) can't be set via wrapETH2LD.
            Fuse::PARENT_CANNOT_CONTROL.bit(),
            "0x231b0Ee14048e9dCcD1d247744d114a4EB5E8E63",
        )
        .unwrap_err();
        assert!(matches!(err, NameWrapperError::OwnerFusesOverflow));
    }

    #[test]
    fn set_fuses_happy_path() {
        let cd = set_fuses_calldata(
            "sbo3lagent.eth",
            fuses_bitmask(&[Fuse::CANNOT_UNWRAP, Fuse::CANNOT_BURN_FUSES]),
        )
        .unwrap();
        assert_eq!(&cd[..4], &SET_FUSES_SELECTOR);
        assert_eq!(cd.len(), 4 + 64);
        // Last 4 bytes are the fuses bitmask big-endian.
        let fuses_bytes = &cd[cd.len() - 4..];
        let n = u32::from_be_bytes([
            fuses_bytes[0],
            fuses_bytes[1],
            fuses_bytes[2],
            fuses_bytes[3],
        ]);
        assert_eq!(n, 0x0003);
    }

    #[test]
    fn set_fuses_rejects_high_bit_fuses() {
        let err =
            set_fuses_calldata("sbo3lagent.eth", Fuse::PARENT_CANNOT_CONTROL.bit()).unwrap_err();
        assert!(matches!(err, NameWrapperError::OwnerFusesOverflow));
    }

    #[test]
    fn nw_set_subnode_record_happy_path() {
        let cd = nw_set_subnode_record_calldata(
            "sbo3lagent.eth",
            "research-agent",
            "0xdc7EFA6b4Bd77d1a406DE4727F0DF567e597D231",
            "0x231b0Ee14048e9dCcD1d247744d114a4EB5E8E63",
            0,
            // Issue with PARENT_CANNOT_CONTROL + CANNOT_UNWRAP →
            // child sovereign.
            fuses_bitmask(&[Fuse::CANNOT_UNWRAP, Fuse::PARENT_CANNOT_CONTROL]),
            // Expiry = max-ish (year 2200ish).
            7_000_000_000,
        )
        .unwrap();
        assert_eq!(&cd[..4], &NW_SET_SUBNODE_RECORD_SELECTOR);
        // 4 selector + 7 head words (32 each) + label tail
        // "research-agent" = 14 bytes, padded to 32.
        assert_eq!(cd.len(), 4 + 32 * 7 + 32 + 32);
    }

    #[test]
    fn nw_set_subnode_record_accepts_high_bit_parent_fuses() {
        // setSubnodeRecord's fuses field is uint32 (not uint16), so
        // PARENT_CANNOT_CONTROL is settable here even though it
        // can't be set via wrapETH2LD or setFuses.
        let cd = nw_set_subnode_record_calldata(
            "sbo3lagent.eth",
            "x",
            "0x0000000000000000000000000000000000000001",
            "0x231b0Ee14048e9dCcD1d247744d114a4EB5E8E63",
            0,
            Fuse::PARENT_CANNOT_CONTROL.bit(),
            0,
        );
        assert!(cd.is_ok());
    }

    #[test]
    fn name_wrapper_for_returns_per_network() {
        assert_eq!(name_wrapper_for("mainnet"), Some(NAME_WRAPPER_MAINNET));
        assert_eq!(name_wrapper_for("sepolia"), Some(NAME_WRAPPER_SEPOLIA));
        assert!(name_wrapper_for("polygon").is_none());
    }

    #[test]
    fn mainnet_address_pinned_to_canonical_form() {
        assert_eq!(NAME_WRAPPER_MAINNET.len(), 42);
        assert!(NAME_WRAPPER_MAINNET.starts_with("0x"));
    }
}
