//! SBO3L policy: YAML/JSON policy parsing and rule evaluation.

pub mod budget;
pub mod engine;
pub mod expr;
pub mod mev_guard;
pub mod model;
pub mod reputation;
mod util;

pub use reputation::{compute_reputation, Reputation, ReputationRow};

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
