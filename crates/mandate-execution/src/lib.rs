//! Mandate execution: sponsor execution adapters.
//!
//! This crate exposes the [`GuardedExecutor`] trait that every sponsor
//! adapter must implement. The contract is: an executor takes a
//! Mandate-approved [`PolicyReceipt`](mandate_core::receipt::PolicyReceipt)
//! plus the original [`PaymentRequest`](mandate_core::aprp::PaymentRequest)
//! and returns an [`ExecutionReceipt`] that callers can attach to the
//! Mandate audit log.
//!
//! * **Mandate decides.** The receipt is the proof of authorisation.
//! * **Sponsor executes.** Each adapter is a thin wrapper over the partner's
//!   real interface (or a clearly-disclosed local mock when credentials are
//!   not available during the hackathon build).
//!
//! The trait + receipt + error types live in
//! [`mandate_core::execution`] (extracted in PR #48 / P5.1 prereq) so the
//! IP-4 standalone-adapter crate can depend on `mandate-core` alone. They
//! are re-exported here so existing call sites
//! (`mandate-server`, `mandate-cli`, `mandate-mcp`,
//! `demo-agents/research-agent`) keep working without import-path
//! changes.

pub mod keeperhub;
pub mod uniswap;

pub use keeperhub::{KeeperHubExecutor, KeeperHubMode};
pub use mandate_core::execution::{ExecutionError, ExecutionReceipt, GuardedExecutor};
pub use uniswap::{
    evaluate_swap, SwapCheck, SwapPolicy, SwapPolicyOutcome, SwapQuote, SwapToken, UniswapExecutor,
    UniswapMode,
};
