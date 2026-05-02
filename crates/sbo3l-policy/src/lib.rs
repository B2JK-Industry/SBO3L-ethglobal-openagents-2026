//! SBO3L policy: YAML/JSON policy parsing and rule evaluation.

#[cfg(feature = "budget")]
pub mod budget;
pub mod cross_chain_reputation;
pub mod engine;
pub mod expr;
pub mod mev_guard;
pub mod model;
pub mod reputation;
mod util;

pub use cross_chain_reputation::{
    aggregate_reputation, recency_factor, AggregateReputationParams, AggregateReputationReport,
    ChainReputationSnapshot, PerChainContribution,
};
pub use reputation::{compute_reputation, Reputation, ReputationRow};

#[cfg(feature = "budget")]
pub use budget::{BudgetDeny, BudgetTracker};
pub use engine::{decide, Decision, EngineError, Outcome};
pub use mev_guard::{
    evaluate as evaluate_mev_guard, MevGuardConfig, MevGuardConfigError, MevGuardOutcome, Quote,
    SwapIntent,
};
pub use model::{
    AgentSelector, AgentStatus, Budget, BudgetScope, DefaultDecision, Emergency, Policy,
    PolicyParseError, PolicyValidationError, Provider, ProviderStatus, Recipient, RecipientStatus,
    Rule, RuleEffect,
};
