//! Sponsor-execution trait + types.
//!
//! Hosting these in `mandate-core` (rather than `mandate-execution`) is the
//! IP-4 prerequisite from `docs/keeperhub-integration-paths.md`: a future
//! `mandate-keeperhub-adapter` crate can `cargo add mandate-core` and
//! implement [`GuardedExecutor`] without pulling the whole Mandate
//! workspace.
//!
//! `mandate-execution` re-exports these symbols so existing call sites
//! (`mandate-server`, `mandate-cli`, `mandate-mcp`, `demo-agents/research-agent`)
//! continue to compile unchanged.

use crate::aprp::PaymentRequest;
use crate::receipt::{Decision, PolicyReceipt};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("policy receipt rejected: decision={0:?}")]
    NotApproved(Decision),
    #[error("sponsor backend offline: {0}")]
    BackendOffline(String),
    #[error("integration: {0}")]
    Integration(String),
}

#[derive(Debug, Clone)]
pub struct ExecutionReceipt {
    pub sponsor: &'static str,
    pub execution_ref: String,
    pub mock: bool,
    pub note: String,
}

/// Contract every sponsor adapter implements. An executor takes a
/// Mandate-approved [`PolicyReceipt`] plus the original [`PaymentRequest`]
/// and returns an [`ExecutionReceipt`] callers can attach to the audit
/// log.
///
/// * **Mandate decides.** The receipt is the proof of authorisation.
/// * **Sponsor executes.** Each adapter is a thin wrapper over the
///   partner's real interface (or a clearly-disclosed local mock when
///   credentials are not available).
pub trait GuardedExecutor {
    fn sponsor_id(&self) -> &'static str;
    fn execute(
        &self,
        request: &PaymentRequest,
        receipt: &PolicyReceipt,
    ) -> Result<ExecutionReceipt, ExecutionError>;
}
