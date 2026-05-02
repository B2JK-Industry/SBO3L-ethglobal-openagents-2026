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

pub mod ccip_read;
pub mod cross_agent;
pub mod cross_chain;
pub mod durin;
pub mod ens;
pub mod ens_anchor;
pub mod ens_live;
pub mod erc8004;
pub mod reputation_publisher;
pub mod token_gate;
pub mod universal;

pub use ccip_read::{
    decode_gateway_data, decode_gateway_response_body, decode_string_result,
    parse_offchain_lookup_revert, CcipError, GatewayBody, GatewayResponse, OffchainLookup,
    OFFCHAIN_LOOKUP_SELECTOR,
};
pub use cross_agent::{
    build_challenge, sign_challenge, verify_challenge, CrossAgentChallenge, CrossAgentError,
    CrossAgentReject, CrossAgentTrust, PubkeyResolver, SignedChallenge, CHALLENGE_SCHEMA,
    FRESHNESS_WINDOW_MS, PUBKEY_RECORD_KEY, TRUST_SCHEMA,
};
pub use cross_chain::{
    build_set_attestation_calldata, commit_report, compute_eip712_digest, from_text_record,
    sign_attestation, to_text_record, verify_attestation, verify_attestation_with_context,
    verify_consistency, ConsistencyReport, CrossChainAttestation, CrossChainError, KnownChain,
    ATTESTATION_TEXT_KEY, DOMAIN_ANCHOR_CHAIN_ID, DOMAIN_NAME, DOMAIN_VERSION, PUBKEY_TEXT_KEY,
};
pub use durin::{
    build_dry_run as build_durin_dry_run, multicall_calldata, register_calldata, DurinDryRun,
    DurinDryRunSetTextEntry, DurinError, DURIN_DRY_RUN_SCHEMA, MAX_RECORD_VALUE_BYTES,
    MULTICALL_SELECTOR, REGISTER_SELECTOR,
};
pub use ens::{EnsRecords, EnsResolver, OfflineEnsResolver, ResolveError};
pub use ens_anchor::{
    build_envelope, namehash, set_text_calldata, AnchorEnvelope, AnchorError, AnchorMode,
    AnchorParams, EnsNetwork, AUDIT_ROOT_KEY, ENVELOPE_SCHEMA_ID, SET_TEXT_SELECTOR,
};
pub use ens_live::{
    JsonRpcTransport, LiveEnsResolver, ReqwestTransport, RpcError, ENS_REGISTRY_ADDRESS,
    RESOLVER_SELECTOR, SBO3L_TEXT_KEYS, TEXT_SELECTOR,
};
pub use erc8004::{
    build_dry_run as build_erc8004_dry_run, register_agent_calldata,
    ChainConfig as Erc8004ChainConfig, Erc8004DryRun, Erc8004Error,
    RegisterRequest as Erc8004RegisterRequest, ERC8004_DRY_RUN_SCHEMA, REGISTER_AGENT_SELECTOR,
};
pub use reputation_publisher::{
    build_publish_envelope, PublishMode, ReputationEventInput, ReputationPublishEnvelope,
    ReputationPublishParams, REPUTATION_ENVELOPE_SCHEMA_ID, REPUTATION_TEXT_KEY,
};
pub use token_gate::{
    risk_class_high, risk_class_low, risk_class_medium, AllOfGates, AnyOfGates, Erc1155Gate,
    Erc721Gate, GateError, GateResult, RiskClass, TokenGate, ERC1155_BALANCE_OF_SELECTOR,
    ERC721_BALANCE_OF_SELECTOR, ERC721_OWNER_OF_SELECTOR,
};
pub use universal::{
    dns_encode, is_offchain_lookup_revert, UniversalError, UniversalResolver,
    UNIVERSAL_RESOLVER_MAINNET, UNIVERSAL_RESOLVER_SEPOLIA, UNIVERSAL_RESOLVE_SELECTOR,
};
