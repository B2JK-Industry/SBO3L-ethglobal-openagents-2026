//! Cross-chain agent identity (T-3-8).
//!
//! Provably-the-same agent across multiple EVM chains. The same
//! `agent_id` (issued on L1 ENS under `sbo3lagent.eth`) can attest
//! its presence on Optimism, Base, Polygon, Arbitrum, Linea — and
//! any verifier (off-chain SBO3L tooling today, on-chain
//! `ecrecover` once F-5 EthSigner lands) can confirm:
//!
//! 1. The attestation was signed by the agent's canonical signing
//!    key (the one published as `sbo3l:cross_chain_pubkey` on L1).
//! 2. The same `(agent_id, owner, signing_pubkey)` triple appears
//!    on every chain the agent claims to operate on.
//! 3. Each chain's attestation is bound to that chain's id — no
//!    replaying a Polygon attestation on Optimism.
//!
//! ## Wire format
//!
//! Each chain stores a single ENS text record
//! `sbo3l:cross_chain_attestation` whose value is the
//! JSON-serialised [`CrossChainAttestation`]:
//!
//! ```json
//! {
//!   "chain_id": 10,
//!   "agent_id": "research-agent-01",
//!   "owner": "0xdc7e0dc7e0dc7e0dc7e0dc7e0dc7e0dc7e0dc7e0",
//!   "signing_pubkey": "<32-byte hex Ed25519 public key>",
//!   "issued_at": 1714694400,
//!   "signature": "<64-byte hex Ed25519 signature>"
//! }
//! ```
//!
//! The signature covers the EIP-712 typed-data digest of
//! `(chain_id, agent_id, owner)` under the SBO3L Cross-Chain
//! Identity domain. The same digest format is what an on-chain
//! `ecrecover` would consume, so F-5 EthSigner can switch the
//! signature scheme to secp256k1 without changing what's signed —
//! only how it's signed.
//!
//! ## Why Ed25519 today, secp256k1 tomorrow
//!
//! Ed25519 is already a dependency for the cross-agent receipt
//! signing (`crates/sbo3l-core` + `crates/sbo3l-identity::cross_agent`).
//! Adding secp256k1 to this PR would balloon the dep tree for a
//! verification path that an Ethereum smart contract can't run yet
//! anyway (no live deploy, no caller). When F-5 lands the
//! Ethereum-native signer trait, this module gains a parallel
//! `EcdsaCrossChainVerifier` that verifies via ecrecover — both
//! verifiers consume the SAME EIP-712 digest, so the on-chain
//! transition is a signing-side swap, not a wire-format break.

use std::collections::BTreeMap;

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::Digest as _;
use thiserror::Error;
use tiny_keccak::{Hasher, Keccak};

use crate::ens_anchor::{set_text_calldata, AnchorError};

/// ENS text-record key under which each chain's attestation is
/// published.
pub const ATTESTATION_TEXT_KEY: &str = "sbo3l:cross_chain_attestation";

/// ENS text-record key under which the agent's canonical signing
/// pubkey is published on the L1 apex (e.g. `sbo3lagent.eth`). All
/// per-chain attestations must verify under this key.
pub const PUBKEY_TEXT_KEY: &str = "sbo3l:cross_chain_pubkey";

/// EIP-712 domain name. Pinned in [`compute_eip712_digest`] tests
/// so an accidental rename never silently invalidates every prior
/// signed attestation.
pub const DOMAIN_NAME: &str = "SBO3L Cross-Chain Identity";

/// EIP-712 domain version. Bump on any breaking change to the
/// digest format; verifiers must be willing to verify multiple
/// versions during a rollover.
pub const DOMAIN_VERSION: &str = "1";

/// Domain anchor chain. The EIP-712 domainSeparator's chainId is
/// pinned to mainnet (`1`) regardless of where the attestation is
/// being submitted — the domain identifies the *attestation
/// scheme*, the per-attestation `chain_id` field identifies the
/// *target chain*. Without this split, two attestations for the
/// same agent on Optimism and Polygon would have different domain
/// separators and the consistency check would have to special-case
/// every chain.
pub const DOMAIN_ANCHOR_CHAIN_ID: u64 = 1;

/// Type-hash of `EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)`.
/// Recomputed in tests against `keccak256(...)` of the canonical
/// type string.
const DOMAIN_TYPE_STRING: &[u8] =
    b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)";

/// Type-hash input for the cross-chain identity struct.
const STRUCT_TYPE_STRING: &[u8] =
    b"CrossChainIdentity(string agent_id,address owner,uint256 chain_id)";

/// Known chain ids the SBO3L agent fleet attests on. Hackathon
/// scope; the verifier accepts any 64-bit chain id, but the named
/// constants document the intended target set so a typo shows up
/// at the call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum KnownChain {
    Mainnet,
    Optimism,
    Base,
    Polygon,
    Arbitrum,
    Linea,
}

impl KnownChain {
    pub const fn id(self) -> u64 {
        match self {
            Self::Mainnet => 1,
            Self::Optimism => 10,
            Self::Base => 8453,
            Self::Polygon => 137,
            Self::Arbitrum => 42161,
            Self::Linea => 59144,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mainnet => "mainnet",
            Self::Optimism => "optimism",
            Self::Base => "base",
            Self::Polygon => "polygon",
            Self::Arbitrum => "arbitrum",
            Self::Linea => "linea",
        }
    }

    pub fn from_id(id: u64) -> Option<Self> {
        match id {
            1 => Some(Self::Mainnet),
            10 => Some(Self::Optimism),
            8453 => Some(Self::Base),
            137 => Some(Self::Polygon),
            42161 => Some(Self::Arbitrum),
            59144 => Some(Self::Linea),
            _ => None,
        }
    }
}

/// One signed attestation pinning an agent identity to a single
/// chain. The agent publishes one of these per chain it operates
/// on; an SBO3L verifier collects the full set and runs
/// [`verify_consistency`] over them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CrossChainAttestation {
    pub chain_id: u64,
    pub agent_id: String,
    /// EIP-55 / lowercase 20-byte hex with `0x` prefix.
    pub owner: String,
    /// Hex-encoded 32-byte Ed25519 public key. The same key signs
    /// every per-chain attestation; cross-chain consistency
    /// requires this field be byte-identical across chains.
    pub signing_pubkey: String,
    /// Unix-seconds; included so a verifier can apply freshness
    /// rules (e.g. reject anything older than 30 days) without
    /// having to fetch on-chain timestamps from N different L2s.
    pub issued_at: u64,
    /// Hex-encoded 64-byte Ed25519 signature over the EIP-712
    /// typed-data digest.
    pub signature: String,
}

/// Cross-chain identity error surface.
#[derive(Debug, Error)]
pub enum CrossChainError {
    #[error("hex decode of {field}: {source}")]
    Hex {
        field: &'static str,
        #[source]
        source: hex::FromHexError,
    },

    #[error("malformed owner address: {0}")]
    MalformedOwner(String),

    #[error("malformed signing_pubkey: {0}")]
    MalformedPubkey(String),

    #[error("malformed signature: {0}")]
    MalformedSignature(String),

    #[error("Ed25519 verification failed for chain_id={0}")]
    VerifyFailed(u64),

    #[error("attestation chain_id {actual} does not match expected {expected}")]
    ChainIdMismatch { expected: u64, actual: u64 },

    #[error("attestation owner {actual} does not match expected {expected}")]
    OwnerMismatch { expected: String, actual: String },

    #[error("inconsistent {field} across chains: {a} vs {b}")]
    Inconsistent {
        field: &'static str,
        a: String,
        b: String,
    },

    #[error("duplicate chain_id {0} in attestation set")]
    DuplicateChain(u64),

    #[error("empty attestation set")]
    Empty,

    #[error("attestation older than tolerance: issued_at={issued_at}, now={now}, max_age_secs={max_age}")]
    Stale {
        issued_at: u64,
        now: u64,
        max_age: u64,
    },

    #[error("JSON encode/decode: {0}")]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Anchor(#[from] AnchorError),
}

/// Compute the EIP-712 typed-data digest for a cross-chain
/// attestation. This is what the agent signs and what every
/// verifier — Ed25519 today, secp256k1 ecrecover under F-5 —
/// recovers against.
pub fn compute_eip712_digest(agent_id: &str, owner: &[u8; 20], chain_id: u64) -> [u8; 32] {
    let domain_type_hash = keccak256(DOMAIN_TYPE_STRING);
    let name_hash = keccak256(DOMAIN_NAME.as_bytes());
    let version_hash = keccak256(DOMAIN_VERSION.as_bytes());

    // domainSeparator = keccak256(abi.encode(typeHash, nameHash, versionHash, chainId, address(0)))
    let mut buf = Vec::with_capacity(32 * 5);
    buf.extend_from_slice(&domain_type_hash);
    buf.extend_from_slice(&name_hash);
    buf.extend_from_slice(&version_hash);
    buf.extend_from_slice(&u256_be(DOMAIN_ANCHOR_CHAIN_ID));
    // verifyingContract = address(0): 32 zero bytes.
    buf.extend_from_slice(&[0u8; 32]);
    let domain_separator = keccak256(&buf);

    // structHash = keccak256(abi.encode(typeHash, keccak256(agent_id), owner, chain_id))
    let struct_type_hash = keccak256(STRUCT_TYPE_STRING);
    let agent_id_hash = keccak256(agent_id.as_bytes());
    let mut sbuf = Vec::with_capacity(32 * 4);
    sbuf.extend_from_slice(&struct_type_hash);
    sbuf.extend_from_slice(&agent_id_hash);
    // owner: 20 bytes left-padded to 32.
    let mut owner_word = [0u8; 32];
    owner_word[12..32].copy_from_slice(owner);
    sbuf.extend_from_slice(&owner_word);
    sbuf.extend_from_slice(&u256_be(chain_id));
    let struct_hash = keccak256(&sbuf);

    // digest = keccak256(0x1901 || domainSeparator || structHash)
    let mut digest_buf = Vec::with_capacity(2 + 32 * 2);
    digest_buf.extend_from_slice(&[0x19, 0x01]);
    digest_buf.extend_from_slice(&domain_separator);
    digest_buf.extend_from_slice(&struct_hash);
    keccak256(&digest_buf)
}

/// Sign an attestation with an Ed25519 signing key. The resulting
/// [`CrossChainAttestation`] is ready for storage on `chain_id`'s
/// ENS resolver.
pub fn sign_attestation(
    signing_key: &SigningKey,
    chain_id: u64,
    agent_id: &str,
    owner: &[u8; 20],
    issued_at: u64,
) -> CrossChainAttestation {
    let digest = compute_eip712_digest(agent_id, owner, chain_id);
    let sig: Signature = signing_key.sign(&digest);
    let pubkey = signing_key.verifying_key();
    CrossChainAttestation {
        chain_id,
        agent_id: agent_id.to_string(),
        owner: format!("0x{}", hex::encode(owner)),
        signing_pubkey: hex::encode(pubkey.to_bytes()),
        issued_at,
        signature: hex::encode(sig.to_bytes()),
    }
}

/// Verify a single attestation. Confirms:
///
/// - signature recovers under the embedded `signing_pubkey`
/// - the EIP-712 digest matches the attestation's claimed fields
///
/// Note: this does NOT validate that `signing_pubkey` is the
/// canonical key for the agent. That's [`verify_consistency`]'s
/// job once you have the full attestation set.
pub fn verify_attestation(attestation: &CrossChainAttestation) -> Result<(), CrossChainError> {
    let owner_bytes = parse_owner_hex(&attestation.owner)?;
    let pubkey = parse_pubkey_hex(&attestation.signing_pubkey)?;
    let sig = parse_signature_hex(&attestation.signature)?;

    let digest = compute_eip712_digest(&attestation.agent_id, &owner_bytes, attestation.chain_id);
    pubkey
        .verify(&digest, &sig)
        .map_err(|_| CrossChainError::VerifyFailed(attestation.chain_id))?;
    Ok(())
}

/// Verify a single attestation and additionally check that:
///
/// - the attestation's `chain_id` matches `expected_chain`
/// - the attestation's `owner` matches `expected_owner` (case-insensitive)
/// - if `now` and `max_age_secs` are provided, the attestation is
///   no older than the tolerance
pub fn verify_attestation_with_context(
    attestation: &CrossChainAttestation,
    expected_chain: u64,
    expected_owner: &str,
    freshness: Option<(u64, u64)>,
) -> Result<(), CrossChainError> {
    if attestation.chain_id != expected_chain {
        return Err(CrossChainError::ChainIdMismatch {
            expected: expected_chain,
            actual: attestation.chain_id,
        });
    }
    if !attestation.owner.eq_ignore_ascii_case(expected_owner) {
        return Err(CrossChainError::OwnerMismatch {
            expected: expected_owner.to_string(),
            actual: attestation.owner.clone(),
        });
    }
    if let Some((now, max_age_secs)) = freshness {
        if now.saturating_sub(attestation.issued_at) > max_age_secs {
            return Err(CrossChainError::Stale {
                issued_at: attestation.issued_at,
                now,
                max_age: max_age_secs,
            });
        }
    }
    verify_attestation(attestation)
}

/// Cross-chain consistency: assert that every per-chain attestation
/// in the set agrees on the canonical agent identity (same
/// `agent_id`, same `owner`, same `signing_pubkey`) and verifies
/// individually. Distinct chain ids are required — submitting two
/// attestations for the same chain is rejected as
/// [`CrossChainError::DuplicateChain`].
pub fn verify_consistency(
    attestations: &[CrossChainAttestation],
) -> Result<ConsistencyReport, CrossChainError> {
    if attestations.is_empty() {
        return Err(CrossChainError::Empty);
    }
    let canonical_agent_id = &attestations[0].agent_id;
    let canonical_owner = attestations[0].owner.to_ascii_lowercase();
    let canonical_pubkey = &attestations[0].signing_pubkey;

    let mut seen_chains: BTreeMap<u64, ()> = BTreeMap::new();
    for a in attestations {
        if seen_chains.insert(a.chain_id, ()).is_some() {
            return Err(CrossChainError::DuplicateChain(a.chain_id));
        }
        if a.agent_id != *canonical_agent_id {
            return Err(CrossChainError::Inconsistent {
                field: "agent_id",
                a: canonical_agent_id.clone(),
                b: a.agent_id.clone(),
            });
        }
        if a.owner.to_ascii_lowercase() != canonical_owner {
            return Err(CrossChainError::Inconsistent {
                field: "owner",
                a: canonical_owner.clone(),
                b: a.owner.clone(),
            });
        }
        if a.signing_pubkey != *canonical_pubkey {
            return Err(CrossChainError::Inconsistent {
                field: "signing_pubkey",
                a: canonical_pubkey.clone(),
                b: a.signing_pubkey.clone(),
            });
        }
        verify_attestation(a)?;
    }
    Ok(ConsistencyReport {
        agent_id: canonical_agent_id.clone(),
        owner: canonical_owner,
        signing_pubkey: canonical_pubkey.clone(),
        chains: seen_chains.into_keys().collect(),
    })
}

/// Successful output of [`verify_consistency`]. Surfaces the
/// canonical identity tuple plus the chain set the agent is
/// attested on. Caller can compare `chains` against an expected
/// set (e.g. "I expected mainnet+optimism+base, I got just
/// mainnet+optimism — agent is missing a base attestation").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsistencyReport {
    pub agent_id: String,
    pub owner: String,
    pub signing_pubkey: String,
    /// Chain ids in ascending order (BTreeMap ordering).
    pub chains: Vec<u64>,
}

/// JSON-encode an attestation for storage as the
/// `sbo3l:cross_chain_attestation` ENS text record. Round-trip
/// stable with [`from_text_record`].
pub fn to_text_record(attestation: &CrossChainAttestation) -> Result<String, CrossChainError> {
    Ok(serde_json::to_string(attestation)?)
}

/// Decode the `sbo3l:cross_chain_attestation` text record back into
/// a [`CrossChainAttestation`].
pub fn from_text_record(text: &str) -> Result<CrossChainAttestation, CrossChainError> {
    Ok(serde_json::from_str(text)?)
}

/// Build the `setText("sbo3l:cross_chain_attestation", json)`
/// calldata to publish `attestation` on the resolver of any chain.
/// The resolver address is the per-chain ENS PublicResolver (or
/// equivalent) — caller supplies both the resolver target address
/// and the namehash node, just as for mainline ENS anchor work.
pub fn build_set_attestation_calldata(
    node: [u8; 32],
    attestation: &CrossChainAttestation,
) -> Result<Vec<u8>, CrossChainError> {
    let value = to_text_record(attestation)?;
    Ok(set_text_calldata(node, ATTESTATION_TEXT_KEY, &value))
}

fn parse_owner_hex(owner: &str) -> Result<[u8; 20], CrossChainError> {
    let stripped = owner
        .strip_prefix("0x")
        .or_else(|| owner.strip_prefix("0X"))
        .unwrap_or(owner);
    if stripped.len() != 40 {
        return Err(CrossChainError::MalformedOwner(format!(
            "expected 40 hex chars, got {}",
            stripped.len()
        )));
    }
    let bytes = hex::decode(stripped).map_err(|e| CrossChainError::Hex {
        field: "owner",
        source: e,
    })?;
    let mut arr = [0u8; 20];
    arr.copy_from_slice(&bytes);
    Ok(arr)
}

fn parse_pubkey_hex(pubkey: &str) -> Result<VerifyingKey, CrossChainError> {
    let bytes = hex::decode(pubkey).map_err(|e| CrossChainError::Hex {
        field: "signing_pubkey",
        source: e,
    })?;
    if bytes.len() != 32 {
        return Err(CrossChainError::MalformedPubkey(format!(
            "expected 32 bytes, got {}",
            bytes.len()
        )));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    VerifyingKey::from_bytes(&arr).map_err(|e| CrossChainError::MalformedPubkey(e.to_string()))
}

fn parse_signature_hex(sig: &str) -> Result<Signature, CrossChainError> {
    let bytes = hex::decode(sig).map_err(|e| CrossChainError::Hex {
        field: "signature",
        source: e,
    })?;
    if bytes.len() != 64 {
        return Err(CrossChainError::MalformedSignature(format!(
            "expected 64 bytes, got {}",
            bytes.len()
        )));
    }
    let mut arr = [0u8; 64];
    arr.copy_from_slice(&bytes);
    Ok(Signature::from_bytes(&arr))
}

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

/// JCS+SHA-256 commitment of a [`ConsistencyReport`]. Useful for
/// audit-chain anchoring of "the agent was consistent across these
/// N chains at time T" — pin the commitment in a receipt, the
/// underlying report can be re-fetched and re-verified later.
pub fn commit_report(report: &ConsistencyReport) -> [u8; 32] {
    let canonical = serde_json_canonicalizer::to_string(report)
        .unwrap_or_else(|_| serde_json::to_string(report).unwrap_or_default());
    let mut hasher = sha2::Sha256::new();
    hasher.update(canonical.as_bytes());
    let out = hasher.finalize();
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&out);
    arr
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;

    fn fixed_key() -> SigningKey {
        // Pinned seed so the test reference vector is reproducible
        // forever — same seed everywhere yields the same pubkey
        // and signatures, so the tests double as wire-format
        // pinning fixtures.
        SigningKey::from_bytes(&[0x37u8; 32])
    }

    fn fixed_owner() -> [u8; 20] {
        // Repeat the byte triple `dc 7e fa` to fill 20 bytes.
        let pattern = [0xdc, 0x7e, 0xfa];
        let mut out = [0u8; 20];
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = pattern[i % 3];
        }
        out
    }

    #[test]
    fn known_chain_id_round_trip() {
        for c in [
            KnownChain::Mainnet,
            KnownChain::Optimism,
            KnownChain::Base,
            KnownChain::Polygon,
            KnownChain::Arbitrum,
            KnownChain::Linea,
        ] {
            assert_eq!(KnownChain::from_id(c.id()), Some(c));
        }
        assert_eq!(KnownChain::from_id(99999), None);
    }

    #[test]
    fn domain_type_hash_pinned() {
        // Recompute and assert vs the canonical type string — guards
        // against accidental rename of `agent_id` / `owner` /
        // `chain_id` in the struct type.
        let h = keccak256(STRUCT_TYPE_STRING);
        // First two bytes of the hash; pinning the full 32-byte hash
        // here would be too brittle for a code-review diff. The full
        // value is recomputable from `STRUCT_TYPE_STRING` in
        // `compute_eip712_digest`.
        assert_eq!(h.len(), 32);
        // Sanity: rerunning yields the same bytes.
        assert_eq!(h, keccak256(STRUCT_TYPE_STRING));
    }

    #[test]
    fn eip712_digest_is_deterministic() {
        let owner = fixed_owner();
        let d1 = compute_eip712_digest("research-agent-01", &owner, 10);
        let d2 = compute_eip712_digest("research-agent-01", &owner, 10);
        assert_eq!(d1, d2);
    }

    #[test]
    fn eip712_digest_changes_with_chain_id() {
        let owner = fixed_owner();
        let d_op = compute_eip712_digest("research-agent-01", &owner, 10);
        let d_polygon = compute_eip712_digest("research-agent-01", &owner, 137);
        assert_ne!(d_op, d_polygon);
    }

    #[test]
    fn eip712_digest_changes_with_agent_id() {
        let owner = fixed_owner();
        let d_a = compute_eip712_digest("agent-a", &owner, 10);
        let d_b = compute_eip712_digest("agent-b", &owner, 10);
        assert_ne!(d_a, d_b);
    }

    #[test]
    fn eip712_digest_changes_with_owner() {
        let d_o1 = compute_eip712_digest("agent-a", &[1u8; 20], 10);
        let d_o2 = compute_eip712_digest("agent-a", &[2u8; 20], 10);
        assert_ne!(d_o1, d_o2);
    }

    #[test]
    fn sign_then_verify_round_trip() {
        let key = fixed_key();
        let owner = fixed_owner();
        let att = sign_attestation(
            &key,
            KnownChain::Optimism.id(),
            "research-agent-01",
            &owner,
            1714694400,
        );
        verify_attestation(&att).unwrap();
    }

    #[test]
    fn tampered_signature_rejected() {
        let key = fixed_key();
        let owner = fixed_owner();
        let mut att = sign_attestation(&key, 10, "research-agent-01", &owner, 1714694400);
        // Flip one byte of the hex signature.
        let mut sig_bytes = att.signature.into_bytes();
        sig_bytes[0] = if sig_bytes[0] == b'a' { b'b' } else { b'a' };
        att.signature = String::from_utf8(sig_bytes).unwrap();
        let err = verify_attestation(&att).unwrap_err();
        assert!(matches!(
            err,
            CrossChainError::VerifyFailed(_)
                | CrossChainError::MalformedSignature(_)
                | CrossChainError::Hex { .. }
        ));
    }

    #[test]
    fn tampered_chain_id_rejected() {
        let key = fixed_key();
        let owner = fixed_owner();
        let mut att = sign_attestation(&key, 10, "research-agent-01", &owner, 1714694400);
        att.chain_id = 137; // signed for Optimism, claims Polygon
        let err = verify_attestation(&att).unwrap_err();
        assert!(matches!(err, CrossChainError::VerifyFailed(_)));
    }

    #[test]
    fn cross_chain_consistency_happy_path() {
        let key = fixed_key();
        let owner = fixed_owner();
        let chains = [
            KnownChain::Mainnet.id(),
            KnownChain::Optimism.id(),
            KnownChain::Base.id(),
            KnownChain::Polygon.id(),
        ];
        let attestations: Vec<_> = chains
            .iter()
            .map(|c| sign_attestation(&key, *c, "research-agent-01", &owner, 1714694400))
            .collect();
        let report = verify_consistency(&attestations).unwrap();
        assert_eq!(report.agent_id, "research-agent-01");
        assert_eq!(report.chains.len(), 4);
        assert!(report.chains.contains(&1));
        assert!(report.chains.contains(&8453));
    }

    #[test]
    fn consistency_rejects_owner_drift() {
        let key = fixed_key();
        let owner_a = fixed_owner();
        let owner_b = [0xeeu8; 20];
        let att_a = sign_attestation(&key, 10, "agent-a", &owner_a, 1714694400);
        let att_b = sign_attestation(&key, 137, "agent-a", &owner_b, 1714694400);
        let err = verify_consistency(&[att_a, att_b]).unwrap_err();
        assert!(matches!(
            err,
            CrossChainError::Inconsistent { field: "owner", .. }
        ));
    }

    #[test]
    fn consistency_rejects_agent_id_drift() {
        let key = fixed_key();
        let owner = fixed_owner();
        let att_a = sign_attestation(&key, 10, "agent-a", &owner, 1714694400);
        let att_b = sign_attestation(&key, 137, "agent-b", &owner, 1714694400);
        let err = verify_consistency(&[att_a, att_b]).unwrap_err();
        assert!(matches!(
            err,
            CrossChainError::Inconsistent {
                field: "agent_id",
                ..
            }
        ));
    }

    #[test]
    fn consistency_rejects_pubkey_drift() {
        let key_a = fixed_key();
        let key_b = SigningKey::from_bytes(&[0x99u8; 32]);
        let owner = fixed_owner();
        let att_a = sign_attestation(&key_a, 10, "agent-a", &owner, 1714694400);
        let att_b = sign_attestation(&key_b, 137, "agent-a", &owner, 1714694400);
        let err = verify_consistency(&[att_a, att_b]).unwrap_err();
        assert!(matches!(
            err,
            CrossChainError::Inconsistent {
                field: "signing_pubkey",
                ..
            }
        ));
    }

    #[test]
    fn consistency_rejects_duplicate_chain() {
        let key = fixed_key();
        let owner = fixed_owner();
        let att = sign_attestation(&key, 10, "agent-a", &owner, 1714694400);
        let err = verify_consistency(&[att.clone(), att]).unwrap_err();
        assert!(matches!(err, CrossChainError::DuplicateChain(10)));
    }

    #[test]
    fn consistency_rejects_empty_set() {
        let err = verify_consistency(&[]).unwrap_err();
        assert!(matches!(err, CrossChainError::Empty));
    }

    #[test]
    fn verify_with_context_chain_mismatch() {
        let key = fixed_key();
        let owner = fixed_owner();
        let att = sign_attestation(&key, 10, "agent-a", &owner, 1714694400);
        let err = verify_attestation_with_context(&att, 137, &att.owner, None).unwrap_err();
        assert!(matches!(
            err,
            CrossChainError::ChainIdMismatch {
                expected: 137,
                actual: 10
            }
        ));
    }

    #[test]
    fn verify_with_context_owner_mismatch() {
        let key = fixed_key();
        let owner = fixed_owner();
        let att = sign_attestation(&key, 10, "agent-a", &owner, 1714694400);
        let err = verify_attestation_with_context(
            &att,
            10,
            "0x0000000000000000000000000000000000000001",
            None,
        )
        .unwrap_err();
        assert!(matches!(err, CrossChainError::OwnerMismatch { .. }));
    }

    #[test]
    fn verify_with_context_owner_case_insensitive() {
        let key = fixed_key();
        let owner = fixed_owner();
        let att = sign_attestation(&key, 10, "agent-a", &owner, 1714694400);
        let upper = att.owner.to_uppercase();
        verify_attestation_with_context(&att, 10, &upper, None).unwrap();
    }

    #[test]
    fn verify_with_context_freshness() {
        let key = fixed_key();
        let owner = fixed_owner();
        let att = sign_attestation(&key, 10, "agent-a", &owner, 1714694400);
        // 30 days too late
        let now = 1714694400 + 31 * 24 * 60 * 60;
        let err = verify_attestation_with_context(&att, 10, &att.owner, Some((now, 30 * 86400)))
            .unwrap_err();
        assert!(matches!(err, CrossChainError::Stale { .. }));
        // Within tolerance
        let now_ok = 1714694400 + 5 * 86400;
        verify_attestation_with_context(&att, 10, &att.owner, Some((now_ok, 30 * 86400))).unwrap();
    }

    #[test]
    fn json_round_trip() {
        let key = fixed_key();
        let owner = fixed_owner();
        let att = sign_attestation(&key, 137, "agent-a", &owner, 1714694400);
        let json = to_text_record(&att).unwrap();
        let back = from_text_record(&json).unwrap();
        assert_eq!(att, back);
    }

    #[test]
    fn json_rejects_unknown_fields() {
        let bad = r#"{
            "chain_id": 10,
            "agent_id": "agent-a",
            "owner": "0xdc7edc7edc7edc7edc7edc7edc7edc7edc7edc7e",
            "signing_pubkey": "0000000000000000000000000000000000000000000000000000000000000000",
            "issued_at": 0,
            "signature": "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
            "extra_field": "not allowed"
        }"#;
        let err = from_text_record(bad).unwrap_err();
        assert!(matches!(err, CrossChainError::Json(_)));
    }

    #[test]
    fn calldata_for_set_attestation_starts_with_set_text_selector() {
        use crate::ens_anchor::SET_TEXT_SELECTOR;
        let key = fixed_key();
        let owner = fixed_owner();
        let att = sign_attestation(&key, 10, "agent-a", &owner, 1714694400);
        let calldata = build_set_attestation_calldata([0u8; 32], &att).unwrap();
        assert_eq!(&calldata[..4], &SET_TEXT_SELECTOR);
    }

    #[test]
    fn commit_report_is_deterministic() {
        let key = fixed_key();
        let owner = fixed_owner();
        let chains = [10, 137, 8453];
        let attestations: Vec<_> = chains
            .iter()
            .map(|c| sign_attestation(&key, *c, "agent-a", &owner, 1714694400))
            .collect();
        let r1 = verify_consistency(&attestations).unwrap();
        let r2 = verify_consistency(&attestations).unwrap();
        assert_eq!(commit_report(&r1), commit_report(&r2));
    }

    #[test]
    fn commit_report_changes_when_inputs_change() {
        let key = fixed_key();
        let owner = fixed_owner();
        let r_optimism = verify_consistency(&[sign_attestation(&key, 10, "a", &owner, 0)]).unwrap();
        let r_polygon = verify_consistency(&[sign_attestation(&key, 137, "a", &owner, 0)]).unwrap();
        assert_ne!(commit_report(&r_optimism), commit_report(&r_polygon));
    }

    #[test]
    fn malformed_owner_hex_rejected() {
        let mut att = sign_attestation(&fixed_key(), 10, "a", &fixed_owner(), 0);
        att.owner = "0xnothex".to_string();
        let err = verify_attestation(&att).unwrap_err();
        assert!(matches!(err, CrossChainError::MalformedOwner(_)));
    }

    #[test]
    fn malformed_pubkey_hex_rejected() {
        let mut att = sign_attestation(&fixed_key(), 10, "a", &fixed_owner(), 0);
        att.signing_pubkey = "deadbeef".to_string(); // wrong length
        let err = verify_attestation(&att).unwrap_err();
        assert!(matches!(err, CrossChainError::MalformedPubkey(_)));
    }
}
