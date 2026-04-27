//! Mandate execution: sponsor execution adapters.
//!
//! This crate exposes the `GuardedExecutor` trait that every sponsor adapter
//! must implement. The contract is: an executor takes a Mandate-approved
//! `PolicyReceipt` plus the original `PaymentRequest` and returns an
//! `ExecutionReceipt` that callers can attach to the Mandate audit log.
//!
//! * **Mandate decides.** The receipt is the proof of authorisation.
//! * **Sponsor executes.** Each adapter is a thin wrapper over the partner's
//!   real interface (or a clearly-disclosed local mock when credentials are
//!   not available during the hackathon build).

pub mod keeperhub;

pub use keeperhub::{KeeperHubExecutor, KeeperHubMode};

use mandate_core::aprp::PaymentRequest;
use mandate_core::receipt::PolicyReceipt;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("policy receipt rejected: decision={0:?}")]
    NotApproved(mandate_core::receipt::Decision),
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

pub trait GuardedExecutor {
    fn sponsor_id(&self) -> &'static str;
    fn execute(
        &self,
        request: &PaymentRequest,
        receipt: &PolicyReceipt,
    ) -> Result<ExecutionReceipt, ExecutionError>;
}
