//! SBO3L identity: ENS agent identity resolution.
//!
//! Resolves an agent's ENS name (e.g., `research-agent.team.eth`) to a set of
//! SBO3L-namespaced text records:
//!
//! * `sbo3l:agent_id`
//! * `sbo3l:endpoint`
//! * `sbo3l:policy_hash`
//! * `sbo3l:audit_root`
//! * `sbo3l:receipt_schema`
//!
//! The trait abstracts the resolution backend so a live testnet ENS lookup and
//! an offline fixture can plug in interchangeably. The hackathon demo defaults
//! to the offline fixture for determinism; a real-resolver implementation can
//! be added without touching call sites.

pub mod ens;

pub use ens::{EnsRecords, EnsResolver, OfflineEnsResolver, ResolveError};
