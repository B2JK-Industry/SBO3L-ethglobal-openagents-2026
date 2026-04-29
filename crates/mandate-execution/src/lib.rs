//! Mandate execution: sponsor execution adapters.
//!
//! This crate is a back-compat aggregator over the per-sponsor adapter
//! crates. The trait + receipt + error types live in
//! [`mandate_core::execution`]; the KeeperHub executor lives in its
//! own publishable crate ([`mandate_keeperhub_adapter`], the IP-4
//! realisation); the Uniswap mock lives here because it has no
//! standalone-publishability story.
//!
//! Existing call sites (`mandate-server`, `mandate-cli`, `mandate-mcp`,
//! `demo-agents/research-agent`) import via this crate's surface
//! unchanged — `mandate_execution::KeeperHubExecutor`,
//! `mandate_execution::GuardedExecutor`, etc. resolve to the same
//! types they always have, just sourced from the IP-4 adapter or
//! `mandate-core` rather than defined in-crate.
//!
//! New third-party consumers who only need the KeeperHub adapter
//! should depend on `mandate-keeperhub-adapter` directly (path/git
//! today; crates.io once published) — that's the IP-4 win.

pub mod uniswap;

pub use mandate_core::execution::{ExecutionError, ExecutionReceipt, GuardedExecutor};
pub use mandate_keeperhub_adapter::{build_envelope, KeeperHubExecutor, KeeperHubMode};
pub use uniswap::{
    evaluate_swap, SwapCheck, SwapPolicy, SwapPolicyOutcome, SwapQuote, SwapToken, UniswapExecutor,
    UniswapMode,
};

/// Back-compat re-export of the old `keeperhub` submodule. Existing
/// callers like `mandate_execution::keeperhub::KeeperHubExecutor` keep
/// resolving without import-path changes; new code should depend on
/// [`mandate_keeperhub_adapter`] directly.
pub mod keeperhub {
    pub use mandate_keeperhub_adapter::{build_envelope, KeeperHubExecutor, KeeperHubMode};
}
