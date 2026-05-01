//! SBO3L execution: sponsor execution adapters.
//!
//! This crate is a back-compat aggregator over the per-sponsor adapter
//! crates. The trait + receipt + error types live in
//! [`sbo3l_core::execution`]; the KeeperHub executor lives in its
//! own publishable crate ([`sbo3l_keeperhub_adapter`], the IP-4
//! realisation); the Uniswap mock lives here because it has no
//! standalone-publishability story.
//!
//! Existing call sites (`sbo3l-server`, `sbo3l-cli`, `sbo3l-mcp`,
//! `demo-agents/research-agent`) import via this crate's surface
//! unchanged — `sbo3l_execution::KeeperHubExecutor`,
//! `sbo3l_execution::GuardedExecutor`, etc. resolve to the same
//! types they always have, just sourced from the IP-4 adapter or
//! `sbo3l-core` rather than defined in-crate.
//!
//! New third-party consumers who only need the KeeperHub adapter
//! should depend on `sbo3l-keeperhub-adapter` directly (path/git
//! today; crates.io once published) — that's the IP-4 win.

pub mod uniswap;
pub mod uniswap_live;
pub mod uniswap_router;
pub mod uniswap_trading;

pub use sbo3l_core::execution::{ExecutionError, ExecutionReceipt, GuardedExecutor};
pub use sbo3l_keeperhub_adapter::{build_envelope, KeeperHubExecutor, KeeperHubMode};
pub use uniswap::{
    evaluate_swap, SwapCheck, SwapPolicy, SwapPolicyOutcome, SwapQuote, SwapToken, UniswapExecutor,
    UniswapMode,
};
pub use uniswap_live::{
    quote_exact_input_single, JsonRpcTransport, LiveConfig, QuoteResult, ReqwestTransport,
    RpcError, QUOTE_EXACT_INPUT_SINGLE_SELECTOR, SEPOLIA_CHAIN_ID, SEPOLIA_QUOTER_V2_ADDRESS,
    SEPOLIA_WETH,
};
pub use uniswap_router::{
    CommandVerdict, EvaluatedCommand, MulticallOutcome, PolicyGate, UniversalRouterCommand,
    UniversalRouterExecutor, UNIVERSAL_ROUTER_MAINNET_V2, UNIVERSAL_ROUTER_SEPOLIA_V2,
};
pub use uniswap_trading::{
    encode_exact_input_single, hex_encode, parse_address, sepolia_etherscan_tx_url, AddressError,
    SwapParams, EXACT_INPUT_SINGLE_SELECTOR, SEPOLIA_SWAP_ROUTER_02, SEPOLIA_USDC,
};

/// Back-compat re-export of the old `keeperhub` submodule. Existing
/// callers like `sbo3l_execution::keeperhub::KeeperHubExecutor` keep
/// resolving without import-path changes; new code should depend on
/// [`sbo3l_keeperhub_adapter`] directly.
pub mod keeperhub {
    pub use sbo3l_keeperhub_adapter::{build_envelope, KeeperHubExecutor, KeeperHubMode};
}
