//! SBO3L identity: ENS agent identity resolution.
//!
//! Resolves an agent's ENS name (e.g., `research-agent.team.eth`) to a set of
//! SBO3L-namespaced text records:
//!
//! * `sbo3l:agent_id`
//! * `sbo3l:endpoint`
//! * `sbo3l:policy_hash`
//! * `sbo3l:audit_root`
//! * `sbo3l:proof_uri`
//!
//! The trait abstracts the resolution backend so a live testnet ENS lookup and
//! an offline fixture can plug in interchangeably. The hackathon demo defaults
//! to the offline fixture for determinism; a real-resolver implementation can
//! be added without touching call sites.

pub mod ens;
pub mod ens_anchor;
pub mod ens_live;
pub mod erc8004;

pub use ens::{EnsRecords, EnsResolver, OfflineEnsResolver, ResolveError};
pub use erc8004::{
    build_dry_run as build_erc8004_dry_run, register_agent_calldata, ChainConfig as Erc8004ChainConfig,
    Erc8004DryRun, Erc8004Error, RegisterRequest as Erc8004RegisterRequest,
    ERC8004_DRY_RUN_SCHEMA, REGISTER_AGENT_SELECTOR,
};
pub use ens_anchor::{
    build_envelope, namehash, set_text_calldata, AnchorEnvelope, AnchorError, AnchorMode,
    AnchorParams, EnsNetwork, AUDIT_ROOT_KEY, ENVELOPE_SCHEMA_ID, SET_TEXT_SELECTOR,
};
pub use ens_live::{
    JsonRpcTransport, LiveEnsResolver, ReqwestTransport, RpcError, ENS_REGISTRY_ADDRESS,
    RESOLVER_SELECTOR, SBO3L_TEXT_KEYS, TEXT_SELECTOR,
};
