//! ERC-8004 Identity Registry — calldata builders + dry-run envelope
//! for registering an SBO3L agent in the on-chain identity registry.
//!
//! T-4-2 ships:
//!
//! 1. **Calldata builders** ([`register_agent_calldata`]) that emit
//!    the ABI bytes for `registerAgent(address,string,string,bytes32)`
//!    on the ERC-8004 Identity Registry. Selector is recompute-pinned
//!    by a unit test so it can never silently drift.
//! 2. An [`Erc8004DryRun`] envelope that bundles the calldata + the
//!    registry address + the metadata URI for the supplied agent.
//! 3. [`build_dry_run`] — the orchestration function. Pure,
//!    deterministic, no chain interaction.
//!
//! Mirrors [`crate::durin`]'s shape for consistency: same dry-run
//! pattern, same `--broadcast not implemented` follow-up roadmap, same
//! [`sbo3l_core::signers::eth::EthSigner`] consumer once broadcast
//! wires up.
//!
//! ## Deployment fallback (Q-T42-1 resolution)
//!
//! Daniel resolved Q-T42-1 (canonical Sepolia ERC-8004 deployment) as
//! **A→B fallback** on 2026-05-01:
//!
//! 1. **A: try canonical first.** If Etherscan's verified-contract
//!    listing at the canonical address has the expected ABI at impl
//!    time, use it. Pin the address + verification link in
//!    `docs/erc8004-integration.md`.
//! 2. **B: deploy reference impl ourselves.** If no canonical exists,
//!    deploy the ENS Labs / ERC-8004-reference contract on Sepolia
//!    (~$3 free testnet gas), pin our deploy tx hash + verified
//!    address.
//!
//! `ChainConfig` exposes the registry address as a const that
//! follow-up PRs can update once Daniel pins the deployment.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tiny_keccak::{Hasher, Keccak};

use crate::ens_anchor::{namehash, AnchorError, EnsNetwork};

/// Function selector for `registerAgent(address,string,string,bytes32)`.
/// Recompute-pinned by [`tests::register_agent_selector_is_canonical`].
pub const REGISTER_AGENT_SELECTOR: [u8; 4] = [0x5a, 0x27, 0xc2, 0x11];

/// Schema id for the dry-run envelope.
pub const ERC8004_DRY_RUN_SCHEMA: &str = "sbo3l.erc8004_dry_run.v1";

/// Per-deploy registry address. Daniel pins at impl time. The
/// placeholder zeros below cause `cast send` to revert harmlessly if
/// an operator forgets to update before broadcast — better than
/// shipping a hardcoded random address that happens to be a real
/// contract.
pub const SEPOLIA_REGISTRY_PLACEHOLDER: [u8; 20] = [0u8; 20];
pub const MAINNET_REGISTRY_PLACEHOLDER: [u8; 20] = [0u8; 20];

#[derive(Debug, Error)]
pub enum Erc8004Error {
    #[error(transparent)]
    Anchor(#[from] AnchorError),

    /// Capsule URI was not a `https://` or `http://` URL.
    #[error("metadata_uri must be http(s); got `{0}`")]
    InvalidMetadataUri(String),

    /// Capsule URI exceeded the recommended on-chain storage cap.
    #[error("metadata_uri is {got} bytes; max 1024 (storage gas budget)")]
    MetadataUriTooLong { got: usize },

    /// Agent address could not be parsed as `0x` + 40 hex chars.
    #[error("agent_address `{0}` is not 0x-prefixed 40-hex-char hex")]
    InvalidAgentAddress(String),

    /// Registry address has not been pinned yet for this network.
    #[error(
        "registry address for network `{0}` is not pinned in this build; \
         update sbo3l_identity::erc8004::ChainConfig once Daniel \
         confirms the canonical deployment or our reference deploy \
         tx hash"
    )]
    RegistryNotPinned(String),
}

/// Per-chain registry config. T-4-2 ships placeholder addresses;
/// follow-up PRs update once the canonical or our-deployed address is
/// known. The zero-address placeholder causes broadcast to revert
/// rather than silently send to a random recipient.
#[derive(Debug, Clone, Copy)]
pub struct ChainConfig {
    pub network: EnsNetwork,
    pub registry: [u8; 20],
}

impl ChainConfig {
    /// Default config for the supplied network. Returns
    /// [`Erc8004Error::RegistryNotPinned`] if the address is the
    /// zero-placeholder, so callers can surface a clear refusal
    /// instead of broadcasting to address(0).
    pub fn for_network(network: EnsNetwork) -> Result<Self, Erc8004Error> {
        let registry = match network {
            EnsNetwork::Sepolia => SEPOLIA_REGISTRY_PLACEHOLDER,
            EnsNetwork::Mainnet => MAINNET_REGISTRY_PLACEHOLDER,
        };
        if registry == [0u8; 20] {
            return Err(Erc8004Error::RegistryNotPinned(
                network.as_str().to_string(),
            ));
        }
        Ok(Self { network, registry })
    }

    /// Construct without the placeholder check — for tests + dry-run
    /// scenarios where the operator wants to preview calldata for a
    /// specific registry address even though our defaults aren't
    /// pinned yet.
    pub fn explicit(network: EnsNetwork, registry: [u8; 20]) -> Self {
        Self { network, registry }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterRequest<'a> {
    /// EVM address that owns the agent. Becomes the on-chain
    /// `agentAddress` in the Identity Registry.
    pub agent_address: [u8; 20],
    /// HTTP(S) URL of the agent's metadata document — for SBO3L this
    /// is the published Passport capsule.
    pub metadata_uri: &'a str,
    /// W3C DID identifying the agent. Defaults to `did:ens:<fqdn>`
    /// when the caller passes `None`.
    pub did: Option<&'a str>,
    /// FQDN whose namehash anchors the agent in ENS — used to
    /// cross-link Identity Registry entry ↔ ENS subname.
    pub ens_fqdn: &'a str,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Erc8004DryRun {
    pub schema: String,
    pub network: String,
    pub registry: String,
    pub agent_address: String,
    pub metadata_uri: String,
    pub did: String,
    pub ens_fqdn: String,
    pub ens_node: String,
    pub register_calldata_hex: String,
    pub broadcasted: bool,
    pub gas_estimate: Option<u64>,
}

/// Build the dry-run envelope. Pure function: deterministic, no IO.
///
/// `did = None` defaults to `did:ens:<fqdn>` per Q-T42-3 resolution
/// (registrant-default identity, leans on our ENS story).
pub fn build_dry_run(
    config: ChainConfig,
    req: RegisterRequest<'_>,
) -> Result<Erc8004DryRun, Erc8004Error> {
    if !req.metadata_uri.starts_with("https://") && !req.metadata_uri.starts_with("http://") {
        return Err(Erc8004Error::InvalidMetadataUri(req.metadata_uri.to_string()));
    }
    if req.metadata_uri.len() > 1024 {
        return Err(Erc8004Error::MetadataUriTooLong {
            got: req.metadata_uri.len(),
        });
    }

    let ens_node = namehash(req.ens_fqdn)?;
    let did_owned;
    let did: &str = match req.did {
        Some(s) => s,
        None => {
            did_owned = format!("did:ens:{}", req.ens_fqdn);
            &did_owned
        }
    };

    let calldata = register_agent_calldata(&req.agent_address, req.metadata_uri, did, &ens_node);

    Ok(Erc8004DryRun {
        schema: ERC8004_DRY_RUN_SCHEMA.to_string(),
        network: config.network.as_str().to_string(),
        registry: format!("0x{}", hex::encode(config.registry)),
        agent_address: format!("0x{}", hex::encode(req.agent_address)),
        metadata_uri: req.metadata_uri.to_string(),
        did: did.to_string(),
        ens_fqdn: req.ens_fqdn.to_string(),
        ens_node: hex::encode(ens_node),
        register_calldata_hex: format!("0x{}", hex::encode(&calldata)),
        broadcasted: false,
        gas_estimate: None,
    })
}

/// ABI-encode `registerAgent(address agentAddress, string metadataUri, string did, bytes32 ensNode)`.
pub fn register_agent_calldata(
    agent_address: &[u8; 20],
    metadata_uri: &str,
    did: &str,
    ens_node: &[u8; 32],
) -> Vec<u8> {
    let metadata_padded = pad_to_32(metadata_uri.len());
    let did_padded = pad_to_32(did.len());

    // 4-byte selector + 4 head words (4*32) + tails for the two
    // strings (length word + padded bytes each).
    let mut out = Vec::with_capacity(
        4 + 4 * 32 + 32 + metadata_padded + 32 + did_padded,
    );
    out.extend_from_slice(&REGISTER_AGENT_SELECTOR);

    // arg 0: address (left-padded 20 bytes to 32).
    out.extend_from_slice(&[0u8; 12]);
    out.extend_from_slice(agent_address);

    // arg 1: string metadataUri — head is offset to tail. Tails start
    // after the 4 head words = 4 * 32 = 0x80.
    let metadata_offset: u64 = 0x80;
    out.extend_from_slice(&u256_be(metadata_offset));

    // arg 2: string did — offset = metadataUri offset + length-word
    // (32) + padded bytes.
    let did_offset = metadata_offset + 32 + metadata_padded as u64;
    out.extend_from_slice(&u256_be(did_offset));

    // arg 3: bytes32 ensNode (inline).
    out.extend_from_slice(ens_node);

    // Tail for arg 1: length word + padded bytes.
    out.extend_from_slice(&u256_be(metadata_uri.len() as u64));
    out.extend_from_slice(metadata_uri.as_bytes());
    out.extend(std::iter::repeat_n(0u8, metadata_padded - metadata_uri.len()));

    // Tail for arg 2: length word + padded bytes.
    out.extend_from_slice(&u256_be(did.len() as u64));
    out.extend_from_slice(did.as_bytes());
    out.extend(std::iter::repeat_n(0u8, did_padded - did.len()));

    out
}

fn pad_to_32(n: usize) -> usize {
    n.div_ceil(32) * 32
}

fn u256_be(n: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[24..].copy_from_slice(&n.to_be_bytes());
    out
}

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

    #[test]
    fn register_agent_selector_is_canonical() {
        let computed = keccak256(b"registerAgent(address,string,string,bytes32)");
        assert_eq!(
            &computed[..4],
            &REGISTER_AGENT_SELECTOR,
            "REGISTER_AGENT_SELECTOR drifted from canonical signature; \
             update both the const and this assertion if the pinned \
             ERC-8004 deployment uses a different signature."
        );
    }

    #[test]
    fn for_network_refuses_zero_placeholder() {
        let err = ChainConfig::for_network(EnsNetwork::Sepolia).unwrap_err();
        assert!(matches!(err, Erc8004Error::RegistryNotPinned(_)));
    }

    #[test]
    fn explicit_skips_placeholder_check() {
        // Real-shaped non-zero address — used in tests where we want
        // to preview calldata against a hypothetical registry.
        let addr = [0x42; 20];
        let cfg = ChainConfig::explicit(EnsNetwork::Sepolia, addr);
        assert_eq!(cfg.registry, addr);
    }

    #[test]
    fn build_dry_run_happy_path() {
        let cfg = ChainConfig::explicit(EnsNetwork::Sepolia, [0x42; 20]);
        let req = RegisterRequest {
            agent_address: [0xaa; 20],
            metadata_uri: "https://example.com/capsule.json",
            did: None,
            ens_fqdn: "research-agent.sbo3lagent.eth",
        };
        let dr = build_dry_run(cfg, req).unwrap();

        assert_eq!(dr.schema, ERC8004_DRY_RUN_SCHEMA);
        assert_eq!(dr.network, "sepolia");
        assert_eq!(dr.registry, "0x4242424242424242424242424242424242424242");
        assert_eq!(dr.agent_address, "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        assert_eq!(dr.did, "did:ens:research-agent.sbo3lagent.eth");
        assert_eq!(dr.ens_fqdn, "research-agent.sbo3lagent.eth");
        assert!(dr.register_calldata_hex.starts_with("0x5a27c211"));
        assert!(!dr.broadcasted);
        assert_eq!(dr.gas_estimate, None);
    }

    #[test]
    fn build_dry_run_explicit_did_overrides_default() {
        let cfg = ChainConfig::explicit(EnsNetwork::Sepolia, [0x42; 20]);
        let req = RegisterRequest {
            agent_address: [0xaa; 20],
            metadata_uri: "https://example.com/capsule.json",
            did: Some("did:ethr:0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            ens_fqdn: "research-agent.sbo3lagent.eth",
        };
        let dr = build_dry_run(cfg, req).unwrap();
        assert_eq!(dr.did, "did:ethr:0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    }

    #[test]
    fn rejects_non_http_metadata_uri() {
        let cfg = ChainConfig::explicit(EnsNetwork::Sepolia, [0x42; 20]);
        let req = RegisterRequest {
            agent_address: [0xaa; 20],
            metadata_uri: "ipfs://Qm...",
            did: None,
            ens_fqdn: "research-agent.sbo3lagent.eth",
        };
        let err = build_dry_run(cfg, req).unwrap_err();
        assert!(matches!(err, Erc8004Error::InvalidMetadataUri(_)));
    }

    #[test]
    fn rejects_oversized_metadata_uri() {
        let cfg = ChainConfig::explicit(EnsNetwork::Sepolia, [0x42; 20]);
        let big = format!("https://example.com/{}", "x".repeat(1024));
        let req = RegisterRequest {
            agent_address: [0xaa; 20],
            metadata_uri: &big,
            did: None,
            ens_fqdn: "research-agent.sbo3lagent.eth",
        };
        let err = build_dry_run(cfg, req).unwrap_err();
        assert!(matches!(err, Erc8004Error::MetadataUriTooLong { .. }));
    }
}
