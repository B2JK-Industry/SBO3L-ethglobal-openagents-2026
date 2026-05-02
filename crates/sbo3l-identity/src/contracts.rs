//! Canonical pinned contract addresses (ENS Track closeout).
//!
//! Single source of truth for every contract address SBO3L either
//! deploys, depends on, or read-side-resolves against. Three goals:
//!
//! 1. **Discoverability** — a reader looking for "the deployed
//!    OffchainResolver address" finds it here, not scattered across
//!    `ens_live.rs`, `universal.rs`, `erc8004.rs`, deploy scripts,
//!    and judge docs.
//! 2. **Audit reproducibility** — the same constants are pinned in
//!    `docs/proof/etherscan-link-pack.md` and surfaced through
//!    `cast code` smoke tests so a judge can independently
//!    verify "yes, this contract is deployed at this address."
//! 3. **Migration safety** — an ENS upgrade (or our own redeploy)
//!    is a one-line constant change here, with a `git blame` that
//!    makes the rotation visible to reviewers.
//!
//! The addresses are grouped by *role* (registry, resolver, signer,
//! reputation registry) and indexed by [`Network`]. Sepolia is
//! covered first-class because every live integration test runs
//! against it; mainnet is covered for production-shaped reads.
//!
//! ## What is and isn't pinned here
//!
//! - **In scope:** every well-known ENS contract we read from
//!   (registry, public resolver, universal resolver), every contract
//!   SBO3L itself deployed (OffchainResolver), every ERC-8004
//!   identity-registry candidate address (placeholder until
//!   Daniel's Sepolia deploy lands), and the deprecated Durin
//!   registrar pointer kept as a historical breadcrumb.
//! - **Out of scope:** transient deploy-script fixtures, per-test
//!   ad-hoc addresses (those stay in their tests for locality), and
//!   wallet-owner addresses (those are operator state, not
//!   protocol state, and live in operator config).

use crate::ens_anchor::EnsNetwork;

/// EVM network whose contract surface SBO3L exercises.
///
/// Mirrors [`EnsNetwork`] for the ENS-bearing chains; other rows
/// (e.g. Optimism for cross-chain attestations) are gated behind
/// future ENSIPs and not pinned here yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Network {
    Mainnet,
    Sepolia,
}

impl From<EnsNetwork> for Network {
    fn from(n: EnsNetwork) -> Self {
        match n {
            EnsNetwork::Mainnet => Self::Mainnet,
            EnsNetwork::Sepolia => Self::Sepolia,
        }
    }
}

impl Network {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mainnet => "mainnet",
            Self::Sepolia => "sepolia",
        }
    }

    pub const fn chain_id(self) -> u64 {
        match self {
            Self::Mainnet => 1,
            Self::Sepolia => 11155111,
        }
    }
}

/// One entry in the address pin table. Carries the raw address +
/// metadata a judge / auditor needs to verify "yes this contract is
/// deployed and is what we claim it is."
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractPin {
    /// `0x`-prefixed 40-hex-char address, EIP-55 mixed case where
    /// the canonical source ships it that way; lowercased otherwise.
    /// Comparisons should be case-insensitive (callers can use
    /// [`addr_eq`]).
    pub address: &'static str,
    /// Network this address is deployed on.
    pub network: Network,
    /// Free-form short label suitable for a verification table
    /// row.
    pub label: &'static str,
    /// Source-of-truth URL — either Etherscan, the ENS docs page
    /// the address is canonical at, or our own deploy-record file.
    pub canonical_source: &'static str,
}

// ============================================================
// ENS infrastructure (read by SBO3L; not deployed by SBO3L)
// ============================================================

/// ENS Registry. Same address across mainnet and Sepolia (and any
/// other chain ENS has been deployed on). Public infrastructure;
/// hardcoded across the ENS ecosystem.
pub const ENS_REGISTRY: ContractPin = ContractPin {
    address: "0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e",
    network: Network::Mainnet, // also deployed verbatim on Sepolia
    label: "ENS Registry (mainnet + Sepolia, same address)",
    canonical_source: "https://docs.ens.domains/learn/contract-api-reference/ens",
};

/// ENS Public Resolver v3 (mainnet). Default resolver for names
/// that haven't customised; reads & writes the standard text-record
/// surface SBO3L's own records live in.
pub const PUBLIC_RESOLVER_MAINNET: ContractPin = ContractPin {
    address: "0x231b0Ee14048e9dCcD1d247744d114a4EB5E8E63",
    network: Network::Mainnet,
    label: "ENS PublicResolver (mainnet)",
    canonical_source: "https://docs.ens.domains/learn/contract-api-reference/publicresolver",
};

/// ENS Public Resolver (Sepolia).
pub const PUBLIC_RESOLVER_SEPOLIA: ContractPin = ContractPin {
    address: "0x8FADE66B79cC9f707aB26799354482EB93a5B7dD",
    network: Network::Sepolia,
    label: "ENS PublicResolver (Sepolia)",
    canonical_source: "https://docs.ens.domains/learn/contract-api-reference/publicresolver",
};

/// Mainnet ENS Universal Resolver v1.x (latest stable as of 2026 Q2).
/// Same constant `viem` ships with. Drives the single-RPC batch read
/// path in [`crate::universal`] (T-4-5).
pub const UNIVERSAL_RESOLVER_MAINNET: ContractPin = ContractPin {
    address: "0xce01f8eee7E479C928F8919abD53E553a36CeF67",
    network: Network::Mainnet,
    label: "ENS UniversalResolver (mainnet)",
    canonical_source: "https://docs.ens.domains/learn/protocol#universal-resolver",
};

/// Sepolia ENS Universal Resolver. Pair to [`UNIVERSAL_RESOLVER_MAINNET`].
pub const UNIVERSAL_RESOLVER_SEPOLIA: ContractPin = ContractPin {
    address: "0xc8Af999e38273D658BE1b921b88A9Ddf005769cC",
    network: Network::Sepolia,
    label: "ENS UniversalResolver (Sepolia)",
    canonical_source: "https://docs.ens.domains/learn/protocol#universal-resolver",
};

// ============================================================
// SBO3L deployments (we control the private key)
// ============================================================

/// SBO3L OffchainResolver on Sepolia (T-4-1 deploy, redeployed
/// 2026-05-03 after Heidi UAT-1 caught Bug #2 — see history block
/// below). Sepolia anchor for every CCIP-Read demo. Paired with
/// the gateway URL template
/// `https://sbo3l-ccip.vercel.app/api/{sender}/{data}.json`
/// (verified canonical via `cast call <addr> 'urls(uint256)(string)' 0`).
///
/// Migration plan (mainnet): the same deploy flow with
/// `NETWORK=mainnet SBO3L_ALLOW_MAINNET_TX=1` produces the mainnet
/// counterpart; pin its address as
/// `OFFCHAIN_RESOLVER_MAINNET` once Daniel's deploy lands.
///
/// **Bug #2 deploy history (2026-05-02 → 2026-05-03):**
///
///   - `0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3` — orig 2026-05-02,
///     stored malformed URL `…/api/{sender}/{data}.json/{data}.json}`
///     (forge `--constructor-args` corrupts string literals containing
///     `{}`). Caught live by `sbo3l doctor --extended` (PR #384).
///     **SUPERSEDED.**
///   - `0x87e99508c222c6e419734cacbb6781b8d282b1f6` — redeploy 2026-05-03
///     via Solidity-side `script/DeployOffchainResolver.s.sol` (PR #383).
///     URL template canonical, **CURRENT**.
///   - `0x9FE5B79f0F32a932E6Bd6A1FE94eb1562f2E05c2` — orphan duplicate
///     2026-05-03 (sibling agent attempted `forge create` redeploy
///     before discovering PR #383, hit same `--constructor-args` bug
///     again). **ABANDONED, malformed URL.**
///   - `0x6056253A1d48DDf6d97FEDfF2664Be15913B0BFF` — orphan duplicate
///     2026-05-03 (this agent's redeploy via `--constructor-args-path`,
///     correct URL template, but PR #383 had already shipped first).
///     **ABANDONED, working but redundant.**
///
/// Lesson reinforced: before deploying any contract, check
/// `gh pr list --state all --search "head:agent/dev<N> deploy"` AND
/// the relevant ContractPin on main.
pub const OFFCHAIN_RESOLVER_SEPOLIA: ContractPin = ContractPin {
    address: "0x87e99508c222c6e419734cacbb6781b8d282b1f6",
    network: Network::Sepolia,
    label: "SBO3L OffchainResolver (Sepolia, T-4-1 redeploy 2026-05-03)",
    canonical_source:
        "https://sepolia.etherscan.io/address/0x87e99508c222c6e419734cacbb6781b8d282b1f6",
};

// ============================================================
// SBO3L deployments — final closeout (2026-05-02 post-R14)
// ============================================================

/// SBO3L AnchorRegistry (R9 P6 contract) on Sepolia. Deployed
/// 2026-05-02; verified live by reading `anchorCount(0x00…)` →
/// `0` (fresh state) via PublicNode RPC.
pub const ANCHOR_REGISTRY_SEPOLIA: ContractPin = ContractPin {
    address: "0x4C302ba8349129bd5963A22e3c7a38a246E8f4Ac",
    network: Network::Sepolia,
    label: "SBO3L AnchorRegistry (Sepolia, R9 P6)",
    canonical_source:
        "https://sepolia.etherscan.io/address/0x4C302ba8349129bd5963A22e3c7a38a246E8f4Ac",
};

/// SBO3L SubnameAuction (R13 P3 contract) on Sepolia. Deployed
/// 2026-05-02; verified `auctionCount() = 0`,
/// `MIN_INCREMENT_BPS() = 500`, `MIN_DURATION() = 3600`.
pub const SUBNAME_AUCTION_SEPOLIA: ContractPin = ContractPin {
    address: "0x5dE75E64739A95701367F3Ad592e0b674b22114B",
    network: Network::Sepolia,
    label: "SBO3L SubnameAuction (Sepolia, R13 P3)",
    canonical_source:
        "https://sepolia.etherscan.io/address/0x5dE75E64739A95701367F3Ad592e0b674b22114B",
};

/// SBO3L ReputationBond (R13 P7 contract) on Sepolia. Deployed
/// 2026-05-02; verified `BOND_AMOUNT() = 1e16` (0.01 ETH),
/// `LOCK_PERIOD() = 604800` (7 days), `slasher` and
/// `insuranceBeneficiary` pinned at constructor time.
pub const REPUTATION_BOND_SEPOLIA: ContractPin = ContractPin {
    address: "0x75072217B43960414047c362198A428f0E9793dA",
    network: Network::Sepolia,
    label: "SBO3L ReputationBond (Sepolia, R13 P7)",
    canonical_source:
        "https://sepolia.etherscan.io/address/0x75072217B43960414047c362198A428f0E9793dA",
};

/// SBO3L ReputationRegistry (R11 P1 contract) on Sepolia. Deployed
/// 2026-05-02; verified `tenantSigner(0x00…) = 0x0…0` (no
/// tenant claimed yet).
pub const REPUTATION_REGISTRY_SEPOLIA: ContractPin = ContractPin {
    address: "0x6aA95d8126B6221607245c068483fa5008F36dc2",
    network: Network::Sepolia,
    label: "SBO3L ReputationRegistry (Sepolia, R11 P1)",
    canonical_source:
        "https://sepolia.etherscan.io/address/0x6aA95d8126B6221607245c068483fa5008F36dc2",
};

// ============================================================
// SBO3L candidate / placeholder pins
// ============================================================

/// Sentinel "no contract here" value. Used by smoke tests to
/// distinguish "expected absent" from "unexpectedly absent."
pub const PLACEHOLDER_ZERO: &str = "0x0000000000000000000000000000000000000000";

/// ERC-8004 IdentityRegistry on Sepolia (T-4-2). Driver-deployed
/// 2026-05-02 from `crates/sbo3l-identity/contracts/IdentityRegistry.sol`
/// — minimal reference impl matching the
/// `registerAgent(address,string,string,bytes32)` calldata shape
/// expected by `crates/sbo3l-identity/src/erc8004.rs`. Selector
/// 0x5a27c211 verified on chain.
///
/// Tx: 0xd1e16958260c88c4f61a11989a878bc66757d1da108430a2f07f659164239e60
/// Etherscan: https://sepolia.etherscan.io/address/0x600c10dE2fd5BB8f3F47cd356Bcb80289845Db37
pub const ERC8004_SEPOLIA: ContractPin = ContractPin {
    address: "0x600c10dE2fd5BB8f3F47cd356Bcb80289845Db37",
    network: Network::Sepolia,
    label: "ERC-8004 IdentityRegistry (Sepolia, SBO3L minimal reference impl)",
    canonical_source: "https://eips.ethereum.org/EIPS/eip-8004",
};

/// Backward-compat alias. Older code referenced
/// `ERC8004_SEPOLIA_PLACEHOLDER` from when the address was still
/// pending. The placeholder is now the real deployment;
/// downstream consumers see the real address transparently.
#[deprecated(note = "Use `ERC8004_SEPOLIA` directly; placeholder concept retired 2026-05-02")]
pub const ERC8004_SEPOLIA_PLACEHOLDER: ContractPin = ERC8004_SEPOLIA;

// ============================================================
// Deprecated (kept as historical breadcrumbs)
// ============================================================

/// **DEPRECATED.** Durin registrar (Sepolia). The pre-pivot path
/// before SBO3L moved to direct `setSubnodeRecord` (decision logged
/// in `memory/durin_dropped_2026-05-01.md`). Kept here as a
/// historical pointer; new code should not consume this constant.
#[deprecated(note = "Durin dropped on 2026-05-01; use direct setSubnodeRecord on ENS Registry")]
pub const DURIN_REGISTRAR_SEPOLIA: ContractPin = ContractPin {
    address: "0x0000000000000000000000000000000000000000",
    network: Network::Sepolia,
    label: "Durin registrar (DEPRECATED — pre-pivot)",
    canonical_source: "https://github.com/durin-protocol/contracts",
};

// ============================================================
// API surface
// ============================================================

/// Look up the canonical resolver pin for a network.
pub const fn resolver_for(network: Network) -> ContractPin {
    match network {
        Network::Mainnet => PUBLIC_RESOLVER_MAINNET,
        Network::Sepolia => PUBLIC_RESOLVER_SEPOLIA,
    }
}

/// Look up the canonical Universal Resolver pin for a network.
pub const fn universal_resolver_for(network: Network) -> ContractPin {
    match network {
        Network::Mainnet => UNIVERSAL_RESOLVER_MAINNET,
        Network::Sepolia => UNIVERSAL_RESOLVER_SEPOLIA,
    }
}

/// All contract pins, ordered by network then by deployment
/// recency. Stable iteration order — useful for
/// `docs/proof/etherscan-link-pack.md` regeneration.
pub fn all_pins() -> Vec<ContractPin> {
    vec![
        ENS_REGISTRY,
        PUBLIC_RESOLVER_MAINNET,
        UNIVERSAL_RESOLVER_MAINNET,
        PUBLIC_RESOLVER_SEPOLIA,
        UNIVERSAL_RESOLVER_SEPOLIA,
        OFFCHAIN_RESOLVER_SEPOLIA,
        ANCHOR_REGISTRY_SEPOLIA,
        SUBNAME_AUCTION_SEPOLIA,
        REPUTATION_BOND_SEPOLIA,
        REPUTATION_REGISTRY_SEPOLIA,
        ERC8004_SEPOLIA,
    ]
}

/// Case-insensitive address comparison. EIP-55 mixed-case forms and
/// lowercase forms must compare equal — callers should never
/// `==`-compare address strings directly.
pub fn addr_eq(a: &str, b: &str) -> bool {
    let a_stripped = a
        .strip_prefix("0x")
        .or_else(|| a.strip_prefix("0X"))
        .unwrap_or(a);
    let b_stripped = b
        .strip_prefix("0x")
        .or_else(|| b.strip_prefix("0X"))
        .unwrap_or(b);
    a_stripped.eq_ignore_ascii_case(b_stripped)
}

/// Return true iff `addr` is the placeholder zero / well-known
/// "this contract isn't deployed yet" sentinel. Smoke tests use
/// this to gate the `cast code` assertion appropriately.
///
/// As of 2026-05-02, ERC-8004 IdentityRegistry on Sepolia is real-
/// deployed (see `ERC8004_SEPOLIA`); only the all-zero address is
/// recognised as a placeholder.
pub fn is_placeholder(addr: &str) -> bool {
    addr_eq(addr, PLACEHOLDER_ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every pinned address is a 40-hex-char value (with optional
    /// `0x` prefix). Catches accidental whitespace, malformed
    /// hex, or wrong-length pastes.
    #[test]
    fn every_pin_is_canonical_form() {
        for pin in all_pins() {
            let stripped = pin
                .address
                .strip_prefix("0x")
                .or_else(|| pin.address.strip_prefix("0X"))
                .unwrap_or(pin.address);
            assert_eq!(
                stripped.len(),
                40,
                "{}: expected 40 hex chars, got {} ({})",
                pin.label,
                stripped.len(),
                pin.address
            );
            assert!(
                stripped.chars().all(|c| c.is_ascii_hexdigit()),
                "{}: contains non-hex char ({})",
                pin.label,
                pin.address
            );
        }
    }

    #[test]
    fn no_two_addresses_are_unintentionally_equal() {
        // Allow ENS Registry being identical across networks (it is
        // by design); flag any other accidental collision.
        let pins = all_pins();
        for (i, a) in pins.iter().enumerate() {
            for b in &pins[i + 1..] {
                if addr_eq(a.address, b.address) {
                    // ENS Registry on mainnet vs Sepolia is the only
                    // expected collision (same address everywhere).
                    let registry_collision = addr_eq(a.address, ENS_REGISTRY.address)
                        && addr_eq(b.address, ENS_REGISTRY.address);
                    assert!(
                        registry_collision,
                        "unexpected address collision: {} ({}) == {} ({})",
                        a.label, a.address, b.label, b.address
                    );
                }
            }
        }
    }

    #[test]
    fn resolver_for_returns_per_network_pin() {
        assert_eq!(
            resolver_for(Network::Mainnet).address,
            PUBLIC_RESOLVER_MAINNET.address
        );
        assert_eq!(
            resolver_for(Network::Sepolia).address,
            PUBLIC_RESOLVER_SEPOLIA.address
        );
    }

    #[test]
    fn universal_resolver_for_returns_per_network_pin() {
        assert_eq!(
            universal_resolver_for(Network::Mainnet).address,
            UNIVERSAL_RESOLVER_MAINNET.address
        );
        assert_eq!(
            universal_resolver_for(Network::Sepolia).address,
            UNIVERSAL_RESOLVER_SEPOLIA.address
        );
    }

    #[test]
    fn addr_eq_is_case_insensitive() {
        assert!(addr_eq(
            "0xabcdef0123456789abcdef0123456789abcdef01",
            "0xABCDEF0123456789ABCDEF0123456789ABCDEF01"
        ));
        assert!(addr_eq(
            "abcdef0123456789abcdef0123456789abcdef01",
            "0xABCDEF0123456789ABCDEF0123456789ABCDEF01"
        ));
        assert!(!addr_eq(
            "0xabcdef0123456789abcdef0123456789abcdef01",
            "0xabcdef0123456789abcdef0123456789abcdef02"
        ));
    }

    #[test]
    fn is_placeholder_recognises_known_sentinels() {
        assert!(is_placeholder(PLACEHOLDER_ZERO));
        // Real addresses don't match.
        assert!(!is_placeholder(ERC8004_SEPOLIA.address)); // now real-deployed
        assert!(!is_placeholder(OFFCHAIN_RESOLVER_SEPOLIA.address));
        assert!(!is_placeholder(ENS_REGISTRY.address));
    }

    #[test]
    fn ens_registry_address_matches_existing_module_constant() {
        // `crate::ens_live::ENS_REGISTRY_ADDRESS` is the existing
        // constant scattered into ens_live.rs. The pin here MUST
        // match it; otherwise consumers reading from one source vs
        // the other would diverge.
        assert!(addr_eq(
            ENS_REGISTRY.address,
            crate::ens_live::ENS_REGISTRY_ADDRESS
        ));
    }

    #[test]
    fn universal_resolver_addresses_match_existing_module_constants() {
        assert!(addr_eq(
            UNIVERSAL_RESOLVER_MAINNET.address,
            crate::universal::UNIVERSAL_RESOLVER_MAINNET
        ));
        assert!(addr_eq(
            UNIVERSAL_RESOLVER_SEPOLIA.address,
            crate::universal::UNIVERSAL_RESOLVER_SEPOLIA
        ));
    }

    #[test]
    fn network_chain_id_round_trip() {
        assert_eq!(Network::Mainnet.chain_id(), 1);
        assert_eq!(Network::Sepolia.chain_id(), 11155111);
    }

    #[test]
    fn network_from_ens_network_round_trip() {
        assert_eq!(Network::from(EnsNetwork::Mainnet), Network::Mainnet);
        assert_eq!(Network::from(EnsNetwork::Sepolia), Network::Sepolia);
    }

    /// The OffchainResolver pin matches what the Foundry tests +
    /// CCIP gateway docs reference. Kept as a regression-net so an
    /// accidental constant change here surfaces alongside any other
    /// drift.
    #[test]
    fn offchain_resolver_sepolia_pinned_to_known_deploy() {
        // Updated 2026-05-03 — orig 0x7c69…8c3 superseded after Bug #2
        // redeploy (PR #383, Solidity-script flow). New canonical
        // verified live via `cast call <addr> 'urls(uint256)(string)' 0`
        // → `https://sbo3l-ccip.vercel.app/api/{sender}/{data}.json`.
        assert_eq!(
            OFFCHAIN_RESOLVER_SEPOLIA.address,
            "0x87e99508c222c6e419734cacbb6781b8d282b1f6"
        );
        assert_eq!(OFFCHAIN_RESOLVER_SEPOLIA.network, Network::Sepolia);
    }
}
