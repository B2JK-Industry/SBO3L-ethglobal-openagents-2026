//! Mandate identity: ENS agent identity resolution.
//!
//! Resolves an agent's ENS name (e.g., `research-agent.team.eth`) to a set of
//! Mandate-namespaced text records:
//!
//! * `mandate:agent_id`
//! * `mandate:endpoint`
//! * `mandate:policy_hash`
//! * `mandate:audit_root`
//! * `mandate:receipt_schema`
//!
//! The trait abstracts the resolution backend so a live testnet ENS lookup and
//! an offline fixture can plug in interchangeably. The hackathon demo defaults
//! to the offline fixture for determinism; a real-resolver implementation can
//! be added without touching call sites.

pub mod ens;

pub use ens::{EnsRecords, EnsResolver, OfflineEnsResolver, ResolveError};
